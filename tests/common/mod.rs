// 后端测试公共模块
// 本模块仅包含后端测试辅助代码，不包含前端测试代码

pub mod fixtures;
pub mod generator;
pub mod seeder;
pub mod env;
pub mod database;

#[cfg(test)]
mod tests {
    // 后端测试辅助模块的单元测试
}
