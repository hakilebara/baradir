pub mod bindings {
    wasmtime::component::bindgen!({
        path: "apps/manager/wit/world.wit",
        world: "manager-app",
    });
}
