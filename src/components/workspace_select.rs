use askama::Template;

pub struct Workspace {
    pub id: String,
    pub name: String,
}

#[derive(Template)]
#[template(path = "components/workspace_select.html")]
pub struct WorkspaceSelect {
    pub current_workspace_id: String,
    pub workspaces: Vec<Workspace>,
}
