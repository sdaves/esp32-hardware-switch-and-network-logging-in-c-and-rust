#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    AnalogSampleReady { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    PostAnalog { millivolts: i32 },
    PostAnalogError { millivolts: i32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub last_millivolts: i32,
    pub in_error: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
