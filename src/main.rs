use clickup_time_in_status_analyzer::services::clickup::ClickUpService;

// static TASK: &str = "86aea18zr";
static TASK: &str = "86a8jcehg";
// static TASK: &str = "86aebe0xh";
// static TASK: &str = "86aefze6c";

fn main() {
    let personal_access_token =
        std::env::var("PERSONAL_TOKEN").expect("failed to get PERSONAL_TOKEN env var.");
    let click_up_service = ClickUpService::new(personal_access_token);

    let task = click_up_service.get_task(TASK);
    let result = click_up_service.generate_points_vs_time_spent_analysis(&task);

    println!("{result}")
}
