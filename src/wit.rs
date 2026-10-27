pub mod bindings {
    wasmtime::component::bindgen!({
        path: "manager/wit/world.wit",
        world: "manager-app",
    });
}
