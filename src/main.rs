use clickup_time_in_status_analyzer::{AggregrationMethod, Application};

static TASK: &str = "86aea18zr";
// static TASK: &str = "86a8jcehg";
// static TASK: &str = "86aebe0xh";
// static TASK: &str = "86aefze6c";

fn main() {
    let personal_access_token =
        std::env::var("PERSONAL_TOKEN").expect("failed to get PERSONAL_TOKEN env var.");
    let application = Application::new(AggregrationMethod::Node, personal_access_token);

    let task = application.get_task(TASK);
    let result = application.generate_points_vs_time_spent_analysis(&task);

    println!("{result}")
}
