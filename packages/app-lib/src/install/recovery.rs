use super::events::emit_install_job;
use super::model::{
    InstallCleanup, InstallErrorView, InstallInterruptReason,
    InstallJobDisplay, InstallJobEventKind, InstallJobState, InstallJobStatus,
    InstallPhaseDetails, InstallPhaseId, InstallRequest, InstallTarget,
};
use super::store;
use crate::event::InstancePayloadType;
use crate::event::emit::emit_instance;
use crate::state::State;
use crate::state::instances::adapters::sqlite::instance_rows;

pub(super) async fn clear_staging_dir(job_state: &InstallJobState) {
    let Some(staging_dir) = &job_state.paths.staging_dir else {
        return;
    };
    if let Err(error) = crate::util::io::remove_dir_all(staging_dir).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            path = %staging_dir.display(),
            "Failed to remove install rollback backup: {error}"
        );
    }
}

pub async fn recover_interrupted_jobs(state: &State) -> crate::Result<()> {
    let jobs = store::list_interrupted_candidates(state).await?;

    for job in jobs {
        let job_id = job.id;
        if let Err(error) = recover_interrupted_job(job, state).await {
            tracing::error!(
                "Error recovering interrupted install job {job_id}: {error}"
            );
        }
    }

    Ok(())
}

async fn recover_interrupted_job(
    mut job: store::InstallJobRecord,
    state: &State,
) -> crate::Result<()> {
    if job.state.display.is_none() {
        job.state.display = display_from_request(&job.state);
    }

    if let Some(instance_id) = target_instance_id(&job.state.target)
        && instance_rows::get_instance_by_id(instance_id, &state.pool)
            .await?
            .is_none()
    {
        let canceled_phase = job.state.progress.phase;
        job.state.error = Some(InstallErrorView::from_message(
            "canceled",
            canceled_phase,
            "Install canceled because the instance was deleted",
        ));
        job.state.record_event(InstallJobEventKind::JobCanceled {
            phase: canceled_phase,
        });

        if let Some(record) = store::finish_active(
            job.id,
            InstallJobStatus::Canceled,
            &job.state,
            state,
        )
        .await?
        {
            store::dismiss(job.id, state).await?;
            clear_staging_dir(&job.state).await;
            emit_install_job(&record.snapshot()).await?;
        }

        return Ok(());
    }

    let interrupted_phase = job.state.progress.phase;
    job.state.record_event(InstallJobEventKind::Interrupted {
        reason: InstallInterruptReason::AppClosed,
        phase: interrupted_phase,
    });
    job.state.progress.phase = InstallPhaseId::RollingBack;
    job.state.progress.progress = None;
    job.state.progress.details = InstallPhaseDetails::Empty;
    job.state.error = Some(InstallErrorView::from_message(
        "app_closed",
        interrupted_phase,
        "App closed while install was running",
    ));

    job.state
        .record_event(InstallJobEventKind::RollbackStarted {
            cleanup: job.state.cleanup.clone(),
        });
    let cleanup_succeeded = match apply_cleanup(&job.state, state).await {
        Ok(()) => {
            job.state
                .record_event(InstallJobEventKind::RollbackCompleted);
            clear_deleted_new_instance_id(&mut job.state);
            true
        }
        Err(error) => {
            tracing::error!(
                "Error cleaning up interrupted install job {}: {error}",
                job.id
            );
            job.state.rollback_error = Some(InstallErrorView::from_error(
                "rollback_error",
                InstallPhaseId::RollingBack,
                &error,
                None,
            ));
            job.state.record_event(InstallJobEventKind::RollbackFailed {
                message: error.to_string(),
            });
            false
        }
    };

    if let Some(record) = store::finish_active(
        job.id,
        InstallJobStatus::Interrupted,
        &job.state,
        state,
    )
    .await?
    {
        if cleanup_succeeded {
            clear_staging_dir(&job.state).await;
        }
        emit_install_job(&record.snapshot()).await?;
    }

    Ok(())
}

fn target_instance_id(target: &InstallTarget) -> Option<&str> {
    match target {
        InstallTarget::NewInstance { instance_id } => instance_id.as_deref(),
        InstallTarget::ExistingInstance { instance_id } => Some(instance_id),
    }
}

fn clear_deleted_new_instance_id(job_state: &mut InstallJobState) {
    if matches!(job_state.cleanup, InstallCleanup::DeleteNewInstance { .. }) {
        job_state.target = InstallTarget::NewInstance { instance_id: None };
        job_state.cleanup =
            InstallCleanup::DeleteNewInstance { instance_id: None };
    }
}

fn display_from_request(state: &InstallJobState) -> Option<InstallJobDisplay> {
    match &state.request {
        InstallRequest::CreateInstance { name, icon_path, .. } => {
            Some(InstallJobDisplay {
                title: name.clone(),
                icon: icon_path.clone(),
            })
        }
        InstallRequest::CreateModpackInstance { location, .. } => match location {
            crate::api::pack::install_from::CreatePackLocation::FromVersionId {
                title,
                icon_url,
                ..
            } => Some(InstallJobDisplay {
                title: title.clone(),
                icon: icon_url.clone(),
            }),
            crate::api::pack::install_from::CreatePackLocation::FromFile {
                ..
            } => None,
        },
        InstallRequest::ImportInstance {
            instance_folder, ..
        } => Some(InstallJobDisplay {
            title: instance_folder.clone(),
            icon: None,
        }),
        InstallRequest::DuplicateInstance { .. }
        | InstallRequest::InstallExistingInstance { .. }
        | InstallRequest::InstallPackToExistingInstance { .. } => {
            state.rollback.as_ref().map(|rollback| InstallJobDisplay {
                title: rollback.instance.instance.name.clone(),
                icon: rollback.instance.instance.icon_path.clone(),
            })
        }
    }
}

pub async fn apply_cleanup(
    job_state: &InstallJobState,
    state: &State,
) -> crate::Result<()> {
    match &job_state.cleanup {
        InstallCleanup::DeleteNewInstance { instance_id } => {
            if let Some(instance_id) = instance_id {
                if crate::state::get_instance(instance_id, &state.pool)
                    .await?
                    .is_some()
                {
                    crate::state::remove_instance(instance_id, state).await?;
                }
                if let Err(error) =
                    emit_instance(instance_id, InstancePayloadType::Removed)
                        .await
                {
                    tracing::warn!(
                        "Failed to emit removed instance {instance_id}: {error}"
                    );
                }
            }
        }
        InstallCleanup::RestoreExistingInstance { instance_id } => {
            if let Some(rollback) = &job_state.rollback {
                crate::state::instances::commands::set_instance_install_stage(
                    instance_id,
                    rollback.install_stage,
                    &state.pool,
                )
                .await?;
                if let Err(error) =
                    emit_instance(instance_id, InstancePayloadType::Edited)
                        .await
                {
                    tracing::warn!(
                        "Failed to emit restored instance {instance_id}: {error}"
                    );
                }
            }
        }
    }

    Ok(())
}
