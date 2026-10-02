#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    DbQueryResultReady { success: bool },
    UartTxDone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    SendUart,
    SyncNetwork,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub synced: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
