//! Parent-owned process lifetime. Windows job handles close on app termination.
use tokio::process::Child;
pub struct SupervisedChild {
    pub child: Child,
    #[cfg(windows)]
    _job: std::os::windows::io::OwnedHandle,
}
impl std::ops::Deref for SupervisedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}
impl std::ops::DerefMut for SupervisedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}
pub fn own(child: Child) -> Result<SupervisedChild, crate::MediaError> {
    #[cfg(windows)]
    {
        let mut child = child;
        use std::os::windows::io::{AsRawHandle, FromRawHandle};
        use windows_sys::Win32::System::JobObjects::*;
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            let _ = child.start_kill();
            return Err(crate::MediaError::Unavailable);
        }
        let job = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(raw) };
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            )
        };
        let assigned = child.raw_handle().is_some_and(
            |handle| unsafe { AssignProcessToJobObject(job.as_raw_handle(), handle) } != 0,
        );
        if configured == 0 || !assigned {
            let _ = child.start_kill();
            return Err(crate::MediaError::Unavailable);
        }
        Ok(SupervisedChild { child, _job: job })
    }
    #[cfg(not(windows))]
    {
        Ok(SupervisedChild { child })
    }
}
