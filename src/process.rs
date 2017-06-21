use std::sync::{Mutex, Arc};
use std::process;
use std::thread;
use std::any::Any;
use Vm;
use Type;
use Parameter;
use Object;
use ObjectCell;
use RunArgs;
use Canvas;
use event::Event;

struct ProcessData {
    child: Arc<Mutex<Option<process::Child>>>,
}

enum ProcessUpdate {
    Finished,
    Read([u8; 1024], usize),
}

pub static process_type: Type = Type {
    name: "Process",
    parameters: &[
        Parameter {
            name: "Command",
            runnable: false,
            output: false,
        },
        Parameter {
            name: "Arguments",
            runnable: false,
            output: false,
        },
        Parameter {
            name: "Input",
            runnable: false,
            output: false,
        },
        Parameter {
            name: "Output",
            runnable: false,
            output: true,
        },
    ],
    init: &|o: &mut Object| {
        o.data = Box::new(ProcessData { child: Arc::new(Mutex::new(None)) });
    },
    run: &|vm: &mut Vm, o: &ObjectCell, args: RunArgs| if let Some(command_rc) = args[0].get(0) {
        let command = command_rc.borrow();
        if let Some(command) = command.data.downcast_ref::<String>() {
            let mut command_builder = process::Command::new(command);
            println!("Executing {}", command);
            for arg_rc in args[1].iter() {
                let arg = arg_rc.borrow();
                if let Some(arg) = arg.data.downcast_ref::<String>() {
                    command_builder.arg(arg);
                } else {
                    println!("Argument is not a string!");
                }
            }
            let child = command_builder
                .stdout(process::Stdio::piped())
                .spawn()
                .expect("failed to execute ls");
            let run_id = vm.start_running(o);
            let tx = vm.tx.clone();
            let output_rc = args[3].get(0).unwrap().clone();
            thread::spawn(move || {

                use std::io::Read;
                let mut stdout = &mut child.stdout.unwrap();
                loop {
                    let mut buffer = [0; 1024];
                    match stdout.read(&mut buffer) {
                        Ok(bytes_read) => {
                            if bytes_read == 0 {
                                tx.send(
                                    Event::RunUpdate(run_id, Box::new(ProcessUpdate::Finished)),
                                );
                                break;
                            } else {
                                tx.send(Event::RunUpdate(
                                    run_id,
                                    Box::new(ProcessUpdate::Read(buffer, bytes_read)),
                                ));
                            }
                        }
                        Err(err) => {
                            println!("Error: {}", err);
                            break;
                        }
                    }
                }
                //let output = child.wait_with_output().expect("failed to wait on ls");
                //println!("Result: {}", String::from_utf8(output.stdout).unwrap());
            });
        } else {
            println!("Command is not a string!");
        }
    } else {
        println!("Missing Command argument!");
    },
    update: Some(&|vm: &mut Vm, o: &ObjectCell, data: Box<Any + Send>| {
        let process_update = data.downcast_ref::<ProcessUpdate>().unwrap();
        match process_update {
            &ProcessUpdate::Finished => println!("Received update"),
            &ProcessUpdate::Read(buffer, bytes_read) => {
                println!("Read {} bytes", bytes_read);
            }
        }

    }),
    draw: &|o: &Object, canvas: &mut Canvas| {},
    serialize: &|o: &Object| -> Vec<u8> { Vec::new() },
    deserialize: &|o: &mut Object, data: Vec<u8>| { (process_type.init)(o); },
};
