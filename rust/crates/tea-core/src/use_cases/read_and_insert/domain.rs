#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    AdcRowReady { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    InsertDbRow { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub rows_logged: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
