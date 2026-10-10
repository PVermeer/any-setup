## [0.1.0] - 2026-10-10

### 🚀 Features

- *(flatpak)* Added optional repo setting

### 🐛 Bug Fixes

- *(user_group)* Added manual group adds for ostree systems
## [0.0.1] - 2026-10-07

### 🚀 Features

- *(pages)* Added dynamic page loading
- *(content-page)* Added content page + some refactoring
- *(pages)* Added contents to content page
- *(pages)* Added basic settings page
- *(actions)* Added basic action system
- *(actions)* Added results to action-runner
- *(action-manager)* Full implementation
- *(action-manager)* Implemented action-manager in app
- *(settings)* Implemented actions run for switch
- *(actions)* Added systemd status checks
- *(view)* Implement new sidebar widget
- *(page)* Added sections
- *(view)* Added action runner progress in the sidebar
- *(view)* Added listener for action progress bar
- *(view)* Sidebar task progress button loads task page
- *(task-manager)* Implemented task id checks
- *(tasks)* Added some basic task ui
- *(tasks-page)* Added some ui updates and refactors
- *(task-page)* Added task-view + refactoring
- *(task-progress)* Add total tasks to widget
- *(tasks)* Added fail_allowed
- *(rpm-ostree)* Added rpm-ostree + refactoring systemd + added tests
- *(rpm-ostree)* Added kargs support
- *(systemd)* Added optional 'now' key
- *(systemd)* Added alias handling
- *(actions)* Added undo structure
- *(dbus-query)* Added dbus lib
- *(actions)* Added on_error
- *(task_manager)* Added new TaskStatus::Added
- *(task_manager)* Added dialog on close when tasks are still running
- *(user_group)* Added user group action
- *(settings)* Added spinner
- *(settings)* Switch row now resets if intended status is not reached
- *(pages)* Added cli option to specify pages dir
- *(action_runner)* Try undo after action fails
- *(flatpak)* Added flatpak action
- *(task_progress)* Added error / warning state
- *(task-manager)* Added toast status messages
- *(app-config)* Added config for app setup

### 🐛 Bug Fixes

- *(action-manager)* Don't return in worker thread
- *(view)* Show page when navigating on minimal view
- *(action-manager)* Switch to async-channel crate to prevent a block on app close
- *(settings)* Enable switch when status is done
- *(task-view)* Only listen for own task
- *(task-manager)* Track listeners by run_id
- *(action_runner)* Added UserExecutionContext for user actions running under root
- *(rpm-ostree)* Added elevation for kargs
- *(sidebar)* Task page now loads again on click
- *(task_view_page)* Don't set error if task actuall succeeded
- *(task_progress)* Some fixes to prgress updates
- *(settings)* Corrected wrong state typo
- *(systemd)* Alias is now resolved again
- *(task_view)* Output doesn't rely on the fd anymore but on the exit code

### 💼 Other

- Initial commit
- *(app)* Create applications dir
- *(app)* Install icon in the right place
- *(app)* Only do dev things in dev-container
- Deny unwraps instead of just warn
- *(polkit)* Added a polkit policy file generator
- *(rpm-spec)* Added rpm-spec + tooling
- *(assets)* Removed static config dir
- *(rpm-spec)* Added build deps for rust build
- *(rpm-spec)* Added desktop validator + rpm-spec validate fixes

### 🚜 Refactor

- *(pages)* Merged serde parsing into type struct
- *(action-manager)* Some name changes + optimizations
- *(task-manager)* Action-manager to task-manager
- *(view)* Sidebar task to own module
- *(task-manager)* More name changes to task_manager
- *(pages)* Moved content nav page to pages
- Small fixes and tweaks
- *(action_runner)* Added to_undo
- *(task_manager)* Changed to pkexec with stdin
- *(task_manager)* Elevated runner to own thread
- *(pages)* Use correct dyn type for methods
- *(actions)* Added macro to replace individual IsAction funtion mapping
- *(action_runner)* Changed results to struct + better output messages
- *(user_execution_context)* Changed to Arc
- *(switch_row)* Moved build switch row to own file
- *(settings)* Moved code to switch_row file + fix enable row in debug
- *(app_dirs)* Changed page loading to lowest level system dir

### 🎨 Styling

- *(systemd)* Format

### 🧪 Testing

- *(systemd)* Updated tests

### ⚙️ Miscellaneous Tasks

- *(init)* Project setup
- *(pages)* Added fallback page
- *(app_menu)* Init app menu
- *(dev)* Dev-container updates
- *(dev)* Updated dev-container
- *(dev)* Update dev-container for fedora 44
- *(dev)* Change logging var to AS_LOG
- *(dev)* Remove artificial sleep
- *(rpm-ostree)* Removed compound action
- *(dev)* Dev container updates
- *(actions)* Rename on_error to before_retry
- *(action_runner)* Add default \n to output
- *(dev)* Pretty print debug values in logs
- *(pages)* Added some extra logging
- *(settings)* Made icon optional
- *(switch)* Only warn if switch is not actionable
- *(task_page)* Set 1 line error on task list
- *(build)* Added metainfo file to build
- *(rpm-spec)* Added polkit file
- *(rpm-spec)* Remove desktop file in rpm
- *(dev)* Added version bump to local release
- *(release)* 0.0.1
