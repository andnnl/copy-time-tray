pub mod sync_tab;
pub mod general_tab;
pub mod about_tab;

/// 配置 tab 的统一接口
pub trait ConfigTab {
    /// Tab 显示标题
    fn title(&self) -> &str;

    /// 构建 tab 内容到容器中
    fn build(&self, container: &gtk::Box);

    /// 保存配置时调用
    fn on_save(&self) -> Result<(), String>;

    /// 加载配置时调用
    fn on_load(&self);
}
