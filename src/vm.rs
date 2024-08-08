#[allow(dead_code)]
#[derive(Debug)]
pub struct VmError {
    msg: String,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct ThreadId(u32);

pub enum StateCommand {
    PopCommand(ThreadId),
    PopFrame(ThreadId),
    PopThread,
}

/// A unit of execution. Can return StateCommands to modify the VM state.
pub trait Command {
    fn eval(&self, thread_id: ThreadId) -> Result<Option<StateCommand>, VmError>;
}

#[derive(Default)]
pub struct Frame {
    /// Stack of commands. The last item in the vector is the current command.
    commands: Vec<Box<dyn Command>>,
}

pub struct Thread {
    id: ThreadId,
    /// Frames. Each frame has it's own stack of commands.
    frames: Vec<Frame>,
}

#[derive(Default)]
pub struct VM {
    /// Virtual threads. Each thread has its own stack of frames.
    threads: Vec<Thread>,
}

impl Thread {
    fn eval(&mut self) -> Result<Option<StateCommand>, VmError> {
        let thread_id = self.id;

        if let Some(frame) = self.frames.last_mut() {
            frame.eval(thread_id)
        } else {
            // no more frames
            Ok(Some(StateCommand::PopThread))
        }
    }
}

impl Frame {
    fn eval(&mut self, thread_id: ThreadId) -> Result<Option<StateCommand>, VmError> {
        if let Some(command) = self.commands.last_mut() {
            command.eval(thread_id)
        } else {
            // no more commands
            Ok(Some(StateCommand::PopFrame(thread_id)))
        }
    }
}

impl VM {
    pub fn new() -> Self {
        let frames = vec![Frame {
            commands: vec![
                Box::new(commands::TaskCallCommand {
                    task_name: "second".to_owned(),
                }),
                Box::new(commands::TaskCallCommand {
                    task_name: "first".to_owned(),
                }),
            ],
        }];

        VM {
            threads: vec![Thread {
                id: ThreadId(0),
                frames,
            }],
        }
    }

    pub fn run(&mut self, thread_id: ThreadId) -> Result<(), VmError> {
        loop {
            let thread = self.get_thread_mut(thread_id).ok_or_else(|| VmError {
                msg: format!("Thread {thread_id:?} not found"),
            })?;

            if let Some(command) = thread.eval()? {
                match command {
                    StateCommand::PopCommand(thread_id) => {
                        self.pop_command(thread_id)?;
                    }
                    StateCommand::PopFrame(thread_id) => {
                        self.pop_frame(thread_id)?;
                    }
                    StateCommand::PopThread => {
                        self.pop_thread(thread_id)?;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn get_thread_mut(&mut self, thread_id: ThreadId) -> Option<&mut Thread> {
        self.threads.iter_mut().find(|thread| thread.id == thread_id)
    }

    fn assert_thread_mut(&mut self, thread_id: ThreadId) -> Result<&mut Thread, VmError> {
        self.get_thread_mut(thread_id).ok_or_else(|| VmError {
            msg: format!("Thread {thread_id:?} does not exist"),
        })
    }

    fn assert_current_frame_mut(&mut self, thread_id: ThreadId) -> Result<&mut Frame, VmError> {
        let thread = self.assert_thread_mut(thread_id)?;
        thread.frames.last_mut().ok_or_else(|| VmError {
            msg: format!("Thread {thread_id:?} has no current frame"),
        })
    }

    fn pop_frame(&mut self, thread_id: ThreadId) -> Result<(), VmError> {
        let thread = self.assert_thread_mut(thread_id)?;
        if thread.frames.is_empty() {
            return Err(VmError {
                msg: "No more frames to pop".to_owned(),
            });
        }
        thread.frames.pop();
        Ok(())
    }

    fn pop_command(&mut self, thread_id: ThreadId) -> Result<(), VmError> {
        let frame = self.assert_current_frame_mut(thread_id)?;
        if frame.commands.is_empty() {
            return Err(VmError {
                msg: "No more commands to pop".to_owned(),
            });
        }
        frame.commands.pop();
        Ok(())
    }

    fn pop_thread(&mut self, thread_id: ThreadId) -> Result<(), VmError> {
        let idx = self
            .threads
            .binary_search_by_key(&thread_id, |thread| thread.id)
            .map_err(|_| VmError {
                msg: format!("Can't remove non-existent thread {thread_id:?}"),
            })?;
        self.threads.remove(idx);
        Ok(())
    }
}

mod commands {
    use super::{Command, StateCommand, ThreadId, VmError};

    pub struct TaskCallCommand {
        pub task_name: String,
    }

    impl Command for TaskCallCommand {
        fn eval(&self, thread_id: ThreadId) -> Result<Option<StateCommand>, VmError> {
            println!("[{:?}] {} call!", thread_id, self.task_name);
            Ok(Some(StateCommand::PopCommand(thread_id)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_world() {
        VM::new().run(ThreadId(0)).unwrap();
    }
}
