#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fault {
    #[cfg(feature = "test-faults")]
    PanicInTool,
    FailWriteScopeClose,
    #[cfg(feature = "test-faults")]
    CommitBusyOnce,
    #[cfg(feature = "test-faults")]
    BeginBusyAlways,
}

#[cfg(feature = "test-faults")]
pub(crate) fn active() -> Option<Fault> {
    match std::env::var("CADENCE_MCP_FAULT").ok().as_deref() {
        Some("panic_in_tool") => Some(Fault::PanicInTool),
        Some("fail_write_scope_close") => Some(Fault::FailWriteScopeClose),
        Some("commit_busy_once") => Some(Fault::CommitBusyOnce),
        Some("begin_busy_always") => Some(Fault::BeginBusyAlways),
        _ => None,
    }
}

#[cfg(not(feature = "test-faults"))]
pub(crate) fn active() -> Option<Fault> {
    None
}
