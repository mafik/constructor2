mod vm;
mod http;
mod canvas;
mod json_canvas;

fn main() {
    let mut vm = vm::Vm::new();
    println!("Listening on port {}", http::port());
    println!("Hit Ctrl+C to break.");
    vm.run_forever();
}
