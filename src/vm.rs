#[allow(dead_code)]
#[derive(Debug)]
pub struct VmError {
    msg: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ThreadId(u32);

pub enum StateCommand {
    Continue, // TODO use Option<StateCommand> instead?
    PopCommand,
    PopFrame,
    DeleteThread(ThreadId),
}

pub trait Command {
    fn eval(&self, thread_id: ThreadId) -> Result<StateCommand, VmError>;
}

#[derive(Default)]
pub struct Frame {
    commands: Vec<Box<dyn Command>>,
}

#[derive(Default)]
pub struct VM {
    threads: Vec<Thread>,
}

pub struct Thread {
    id: ThreadId,
    frames: Vec<Frame>,
}

impl Thread {
    fn get_current_frame_mut(&mut self) -> Option<&mut Frame> {
        if self.frames.is_empty() {
            return None;
        }
        let idx = self.frames.len() - 1;
        self.frames.get_mut(idx)
    }

    fn pop_frame(&mut self) -> Result<(), VmError> {
        if self.frames.is_empty() {
            return Err(VmError {
                msg: "No more frames to pop".to_owned(),
            });
        }
        self.frames.pop();
        Ok(())
    }

    fn eval(&mut self) -> Result<StateCommand, VmError> {
        let thread_id = self.id;

        if let Some(frame) = self.get_current_frame_mut() {
            if let Some(command) = frame.get_current_command() {
                match command.eval(thread_id)? {
                    StateCommand::Continue => Ok(StateCommand::Continue),
                    StateCommand::PopCommand => {
                        frame.pop_command()?;
                        Ok(StateCommand::Continue)
                    }
                    StateCommand::PopFrame => {
                        self.pop_frame()?;
                        Ok(StateCommand::Continue)
                    }
                    cmd @ StateCommand::DeleteThread(..) => Ok(cmd),
                }
            } else {
                Ok(StateCommand::DeleteThread(self.id))
            }
        } else {
            Ok(StateCommand::DeleteThread(self.id))
        }
    }
}

impl Frame {
    fn get_current_command(&mut self) -> Option<&dyn Command> {
        if self.commands.is_empty() {
            return None;
        }
        let idx = self.commands.len() - 1;
        self.commands.get(idx).map(|cmd| &**cmd)
    }

    fn pop_command(&mut self) -> Result<(), VmError> {
        if self.commands.is_empty() {
            return Err(VmError {
                msg: "No more commands to pop".to_owned(),
            });
        }
        self.commands.pop();
        Ok(())
    }
}

impl VM {
    pub fn new() -> Self {
        let frames = vec![Frame {
            commands: vec![Box::new(commands::TaskCallCommand {})],
        }];

        VM {
            threads: vec![Thread {
                id: ThreadId(0),
                frames,
            }],
        }
    }

    fn get_thread_mut(&mut self, thread_id: &ThreadId) -> Option<&mut Thread> {
        self.threads
            .iter_mut()
            .find(|thread| thread.id == *thread_id)
    }

    fn delete_thread(&mut self, thread_id: &ThreadId) -> Result<(), VmError> {
        let idx = self
            .threads
            .binary_search_by_key(&thread_id, |thread| &thread.id)
            .map_err(|_| VmError {
                msg: format!("Can't remove non-existent thread {thread_id:?}"),
            })?;
        self.threads.remove(idx);
        Ok(())
    }

    pub fn run(&mut self, thread_id: &ThreadId) -> Result<(), VmError> {
        loop {
            if self.threads.is_empty() {
                break;
            }

            let state_command;
            if let Some(thread) = self.get_thread_mut(thread_id) {
                state_command = Some(thread.eval()?);
                if thread.frames.is_empty() {
                    self.delete_thread(thread_id)?;
                }
            } else {
                break;
            }

            match state_command.unwrap_or(StateCommand::Continue) {
                StateCommand::Continue => {}
                StateCommand::PopCommand => todo!(),
                StateCommand::PopFrame => todo!(),
                StateCommand::DeleteThread(thread_id) => {
                    self.delete_thread(&thread_id)?;
                }
            }
        }
        Ok(())
    }
}

mod commands {
    use super::{Command, StateCommand, ThreadId, VmError};

    pub struct TaskCallCommand {}

    impl Command for TaskCallCommand {
        fn eval(&self, thread_id: ThreadId) -> Result<StateCommand, VmError> {
            println!("[{:?}] Task call!", thread_id);
            Ok(StateCommand::PopCommand)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_world() {
        VM::new().run(&ThreadId(0)).unwrap();
    }
}
