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
            fn run_action(&self, output: &mut Output, user_context: &Arc<UserExecutionContext>, needs_user_contex: bool) -> Result<()> {
                match self {
                    $(
                        Self::$variant(action) => action.run_action(output, user_context, needs_user_contex),
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

            fn get_status(&self, user_context: &Arc<UserExecutionContext>) -> Result<ActionState> {
                match self {
                    $(
                        Self::$variant(action) => action.get_status(user_context),
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

            fn before_retry(&self, output: &mut Output, user_context: &Arc<UserExecutionContext>, needs_user_contex: bool) -> Option<Result<()>> {
                match self {
                    $(
                        Self::$variant(action) => action.before_retry(output, user_context, needs_user_contex),
                    )+
                }
            }

            fn needs_reboot(&self) -> bool {
                match self {
                    $(
                        Self::$variant(action) => action.needs_reboot(),
                    )+
                }
            }
        }
    };
}
