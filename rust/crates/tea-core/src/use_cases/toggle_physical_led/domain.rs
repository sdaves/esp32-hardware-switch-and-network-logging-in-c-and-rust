#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    ButtonDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    None,
    ToggleLed { pin: u8, level: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub led_on: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpdateResult {
    pub next: Model,
    pub command: Cmd,
}
