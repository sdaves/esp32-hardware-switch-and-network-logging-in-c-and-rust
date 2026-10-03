#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    DbQueryResultReady { success: bool },
    UartTxDone,
    LedToggled { on: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    SendUart,
    SyncNetwork { led_on: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub synced: bool,
    pub led_on: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
