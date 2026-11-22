use crate::{Diagnostic, Project, import_ascii, open, save, svg};
use wasm_bindgen::prelude::*;

fn reject(error: Diagnostic) -> JsValue {
    JsValue::from_str(&serde_json::to_string(&error).expect("diagnostics are serializable"))
}

#[wasm_bindgen]
#[derive(Default)]
pub struct Grace {
    project: Project,
}

#[wasm_bindgen]
impl Grace {
    pub fn load_parameters(&mut self, graph: u32, bytes: &[u8]) -> Result<(), JsValue> {
        self.project.load_parameters(graph, bytes).map_err(reject)
    }

    pub fn save_parameters(&self, graph: u32) -> Result<Vec<u8>, JsValue> {
        self.project.save_parameters(graph).map_err(reject)
    }

    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_project(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        self.project = open(bytes).map_err(reject)?;
        Ok(())
    }

    pub fn new_project(&mut self) {
        self.project = Project::new();
    }

    pub fn import_data(&mut self, graph: u32, bytes: &[u8]) -> Result<(), JsValue> {
        import_ascii(&mut self.project, graph, bytes).map_err(reject)
    }

    pub fn import_mapped(
        &mut self,
        graph: u32,
        bytes: &[u8],
        options: &str,
    ) -> Result<(), JsValue> {
        self.project
            .import_mapped(graph, bytes, options)
            .map_err(reject)
    }

    pub fn export_ascii(&self, graph: u32, set: u32) -> Result<Vec<u8>, JsValue> {
        self.project.export_ascii(graph, set).map_err(reject)
    }

    pub fn data_table(
        &self,
        graph: u32,
        set: u32,
        offset: usize,
        limit: usize,
    ) -> Result<String, JsValue> {
        let rows = self
            .project
            .data_table(graph, set, offset, limit)
            .map_err(reject)?;
        Ok(serde_json::to_string(rows).expect("validated numeric rows are serializable"))
    }

    pub fn snapshot(&self) -> String {
        serde_json::to_string(&self.project).expect("project summaries are serializable")
    }

    pub fn render_svg(&self) -> String {
        svg(&self.project)
    }

    pub fn save_project(&self) -> Result<Vec<u8>, JsValue> {
        save(&self.project).map_err(reject)
    }

    pub fn apply(&mut self, command: &str) -> Result<(), JsValue> {
        self.project.apply_command_json(command).map_err(reject)
    }

    pub fn run_script(&mut self, graph: u32, input: &str) -> Result<(), JsValue> {
        self.project.run_script(graph, input).map_err(reject)
    }

    pub fn reload_data(&mut self, graph: u32, set: u32, bytes: &[u8]) -> Result<(), JsValue> {
        self.project.reload_data(graph, set, bytes).map_err(reject)
    }

    pub fn netcdf_variables(&self, bytes: &[u8]) -> Result<String, JsValue> {
        let variables = crate::netcdf_variables(bytes).map_err(reject)?;
        Ok(serde_json::to_string(&variables).expect("variable summaries are serializable"))
    }

    pub fn import_netcdf(
        &mut self,
        graph: u32,
        bytes: &[u8],
        x: &str,
        y: &str,
    ) -> Result<(), JsValue> {
        self.project
            .import_netcdf(graph, bytes, (!x.is_empty()).then_some(x), y)
            .map_err(reject)
    }

    pub fn region_report(&self, graph: u32, id: u32) -> Result<String, JsValue> {
        let counts = self.project.region_report(graph, id).map_err(reject)?;
        Ok(serde_json::to_string(&counts).expect("region counts are serializable"))
    }
}
