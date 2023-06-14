use crossbeam_channel::bounded;
use std::io::Write;
use subprocess::{Popen, PopenConfig, Redirection};

/// These values are used to communicate with a subprocess launched by the `Task` struct.
/// The `Input` variant is used to send input to the subprocess, while
/// the `Exit` variant is used to signal that the subprocess should be terminated.
pub enum Instruction {
    Input(String),
    Exit,
}

/// The `Task` struct represents a task with a command, arguments, communication channels for control
/// and input. It is the primary means for a supervisor to launch a task and channel control instructions
/// to it at runtime.
///
/// Properties:
///
/// * `command`: A string that represents the command to be executed by the task.
/// * `args`: `args` is a vector of strings that represents the arguments for the command that the
/// `Task` struct is associated with. These arguments can be passed to the command when it is executed.
/// * `control`: `control` is a `crossbeam_channel::Sender` that allows sending `Instruction` messages
/// to control the task. This can be used to start, stop, pause, or resume the task, for example.
/// * `input`: `input` is a field of type `crossbeam_channel::Receiver<Instruction>`. It is a channel
/// receiver that can receive messages of type `Instruction`. This field is used to receive instructions
/// from the main thread or other tasks.
pub struct Task {
    command: String,
    args: Vec<String>,
    pub control: crossbeam_channel::Sender<Instruction>,
    input: crossbeam_channel::Receiver<Instruction>,
}

impl Task {
    pub fn new(command: String, args: Vec<String>) -> Task {
        let (command_tx, command_rx) = bounded(10);

        Task {
            command,
            args,
            control: command_tx,
            input: command_rx,
        }
    }

    pub fn launch(&self) -> bool {
        // Configure the Popen command
        let config = PopenConfig {
            stdin: Redirection::Pipe,
            ..Default::default()
        };

        log::debug!(
            "Launching subtask {} with arguments [{:?}]",
            &self.command,
            &self.args
        );
        let pcmd = [self.command.clone()]
            .iter()
            .chain(self.args.iter())
            .cloned()
            .collect::<Vec<_>>();
        let mut handle = Popen::create(&pcmd, config).expect("failed to spawn subprocess");
        let input = self.input.clone();
        // Spawn a thread to manage the subprocess
        std::thread::spawn(move || {
            // Get the subprocess's stdin handle
            let mut stdin = handle.stdin.take().unwrap();

            // Create a buffer to read output from the subprocess
            // let mut stdout = BufReader::new(child.stdout.take().unwrap());

            // Read input from stdin and send it to the subprocess
            loop {
                let instruction = input.recv().unwrap();
                log::debug!("Received instruction: {:#?}", &instruction);
                match instruction {
                    Instruction::Input(input) => {
                        stdin.write_all(input.as_bytes()).unwrap();
                        stdin.flush().unwrap();
                    }
                    Instruction::Exit => {
                        handle.kill().unwrap();
                        break;
                    }
                }
            }

            // Wait for the subprocess to finish and get its output
            match handle.wait().expect("failed to wait for agent subprocess") {
                subprocess::ExitStatus::Exited(status) => {
                    if status != 0 {
                        log::warn!("subprocess exited abnormally with status: {}", status);
                    } else {
                        log::info!("subprocess exited successfully");
                    }
                }
                subprocess::ExitStatus::Signaled(status) => {
                    if status != 0 {
                        log::warn!("subprocess exited abnormally with status: {}", status);
                    } else {
                        log::info!("subprocess exited successfully");
                    }
                }
                subprocess::ExitStatus::Other(_) => todo!(),
                subprocess::ExitStatus::Undetermined => todo!(),
            }
        });
        true
    }
}

#[macro_export]
/// This macro takes in an Option<&str> as a filename and returns its basename without suffix
macro_rules! get_stem {
    ($fname:expr) => {
        $fname
            .and_then(|name| {
                std::path::Path::new(name)
                    .file_stem()
                    .and_then(|stem| stem.to_str())
            })
            .unwrap_or("")
            .to_string();
    };
}
