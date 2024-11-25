use crate::db::models::SolanaProgramBuild;

pub fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(ToOwned::to_owned)
}

pub fn get_repo_url(build_params: &SolanaProgramBuild) -> String {
    build_params.commit_hash.as_ref().map_or_else(
        || build_params.repository.clone(),
        |hash| format!("{}/tree/{}", build_params.repository.trim_end_matches('/'), hash),
    )
}

pub fn extract_hash(output: &str, prefix: &str) -> Option<String> {
    output
        .lines()
        .find(|line| line.starts_with(prefix))
        .map(|line| {
            let hash = line.trim_start_matches(prefix.trim()).trim();
            hash.to_owned()
        })
}
