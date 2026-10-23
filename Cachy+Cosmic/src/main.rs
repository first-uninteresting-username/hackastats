mod app;
mod i18n;

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt::init();
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);
    cosmic::applet::run::<app::AppModel>(())
}
