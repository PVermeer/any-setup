macro_rules! impl_action {
    (
        $enum:ident {
            $(
                $variant:ident
            ),+ $(,)?
        }
    ) => {
        impl Display for $enum {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        Self::$variant(action) => action.fmt(f),
                    )+
                }
            }
        }

        impl IsAction for $enum {
            fn get_command(&self) -> Command {
                match self {
                    $(
                        Self::$variant(action) => action.get_command(),
                    )+
                }
            }

            fn needs_elevation(&self) -> bool {
                match self {
                    $(
                        Self::$variant(action) => action.needs_elevation(),
                    )+
                }
            }

            fn get_status(&self) -> Result<ActionState> {
                match self {
                    $(
                        Self::$variant(action) => action.get_status(),
                    )+
                }
            }

            fn fail_allowed(&self) -> bool {
                match self {
                    $(
                        Self::$variant(action) => action.fail_allowed(),
                    )+
                }
            }

            fn to_undo(&self) -> Self {
                match self {
                    $(
                        Self::$variant(action) => {
                            Self::$variant(action.to_undo())
                        }
                    )+
                }
            }

            fn on_error(&self, output: &Output) -> Option<Command> {
                match self {
                    $(
                        Self::$variant(action) => action.on_error(output),
                    )+
                }
            }
        }
    };
}
