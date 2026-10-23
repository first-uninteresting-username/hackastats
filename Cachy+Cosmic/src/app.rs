use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use cosmic::{
    Element, Task,
    applet::cosmic_panel_config::PanelAnchor,
    iced::{Alignment, Length, Limits, Subscription, window::Id},
    widget::{button, column, container, divider, icon, row, scrollable, space, text},
};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

const APP_ID: &str = "io.github.first_uninteresting_username.Hackastats";
const DEFAULT_API_URL: &str = "https://hackatime.hackclub.com/api/hackatime/v1";
const REFRESH_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct ConnectionConfig {
    base_url: String,
    api_key: String,
    source: String,
}

fn apply_wakatime_config(contents: &str, config: &mut ConnectionConfig) {
    let mut in_settings = false;
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_settings = line.eq_ignore_ascii_case("[settings]");
            continue;
        }
        if !in_settings || line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "api_url" => config.base_url = value.trim().trim_end_matches('/').to_owned(),
            "api_key" => config.api_key = value.trim().to_owned(),
            _ => {}
        }
    }
}

impl ConnectionConfig {
    fn load() -> Self {
        let mut config = Self {
            base_url: std::env::var("HACKASTATS_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_API_URL.to_owned()),
            api_key: std::env::var("HACKASTATS_API_KEY").unwrap_or_default(),
            source: "environment/defaults".to_owned(),
        };

        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".wakatime.cfg");
            if let Ok(contents) = fs::read_to_string(&path) {
                apply_wakatime_config(&contents, &mut config);
                config.source = path.display().to_string();
            }
        }

        config.base_url = config.base_url.trim_end_matches('/').to_owned();
        config
    }

    fn today_endpoint(&self) -> String {
        format!("{}/users/current/statusbar/today", self.base_url)
    }

    fn week_endpoint(&self) -> String {
        format!("{}/users/current/stats/last_7_days", self.base_url)
    }

    fn dashboard_url(&self) -> String {
        let Some((scheme, rest)) = self.base_url.split_once("://") else {
            return "https://hackatime.hackclub.com/dashboard".to_owned();
        };
        let host = rest.split('/').next().unwrap_or(rest);
        format!("{scheme}://{host}/dashboard")
    }
}

#[derive(Debug, Clone, Deserialize)]
struct StatusResponse {
    data: StatusData,
}

#[derive(Debug, Clone, Deserialize)]
struct StatusData {
    grand_total: GrandTotal,
}

#[derive(Debug, Clone, Deserialize)]
struct GrandTotal {
    text: String,
    #[serde(default)]
    total_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct WeekResponse {
    data: WeekData,
}

#[derive(Debug, Clone, Deserialize)]
struct WeekData {
    #[serde(default)]
    human_readable_total: String,
    #[serde(default)]
    human_readable_daily_average: String,
    #[serde(default)]
    languages: Vec<Breakdown>,
    #[serde(default)]
    projects: Vec<Breakdown>,
}

#[derive(Debug, Clone, Deserialize)]
struct Breakdown {
    name: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    percent: f64,
}

#[derive(Debug, Clone)]
pub struct TodayStats {
    text: String,
    total_seconds: f64,
    updated_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct DashboardStats {
    today: TodayStats,
    week: Option<WeekData>,
    week_error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LoadError {
    MissingApiKey,
    Unauthorized,
    Unavailable(String),
}

impl LoadError {
    fn panel_text(&self) -> &'static str {
        match self {
            Self::MissingApiKey => "No key",
            Self::Unauthorized => "Auth error",
            Self::Unavailable(_) => "Offline",
        }
    }

    fn detail(&self) -> String {
        match self {
            Self::MissingApiKey => {
                "Add your Hackatime API key to ~/.wakatime.cfg, then refresh.".to_owned()
            }
            Self::Unauthorized => "Hackatime rejected the API key in ~/.wakatime.cfg.".to_owned(),
            Self::Unavailable(message) => message.clone(),
        }
    }
}

#[derive(Debug, Clone)]
enum Status {
    Loading,
    Ready(DashboardStats),
    Error(LoadError),
}

impl Status {
    fn panel_text(&self) -> String {
        match self {
            Self::Loading => "…".to_owned(),
            Self::Ready(stats) => stats.today.text.clone(),
            Self::Error(error) => error.panel_text().to_owned(),
        }
    }
}

pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    status: Status,
    connection: ConnectionConfig,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Refresh,
    Loaded(Result<DashboardStats, LoadError>),
    OpenDashboard,
}

async fn fetch_dashboard(connection: ConnectionConfig) -> Result<DashboardStats, LoadError> {
    if connection.api_key.trim().is_empty() {
        return Err(LoadError::MissingApiKey);
    }

    let authorization = format!("Basic {}", BASE64.encode(connection.api_key.as_bytes()));
    let client = Client::new();
    let response = client
        .get(connection.today_endpoint())
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::AUTHORIZATION, authorization.clone())
        .send()
        .await
        .map_err(|error| LoadError::Unavailable(format!("Could not reach Hackatime: {error}")))?;

    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(LoadError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(LoadError::Unavailable(format!(
            "Hackatime returned HTTP {}.",
            response.status()
        )));
    }

    let payload = response
        .json::<StatusResponse>()
        .await
        .map_err(|error| LoadError::Unavailable(format!("Invalid Hackatime response: {error}")))?;
    let today = TodayStats {
        text: payload.data.grand_total.text,
        total_seconds: payload.data.grand_total.total_seconds,
        updated_at: SystemTime::now(),
    };

    let week_response = client
        .get(connection.week_endpoint())
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::AUTHORIZATION, authorization)
        .send()
        .await;
    let (week, week_error) = match week_response {
        Ok(response) if response.status().is_success() => {
            match response.json::<WeekResponse>().await {
                Ok(payload) => (Some(payload.data), None),
                Err(error) => (
                    None,
                    Some(format!("Weekly stats could not be decoded: {error}")),
                ),
            }
        }
        Ok(response) => (
            None,
            Some(format!("Weekly stats returned HTTP {}.", response.status())),
        ),
        Err(error) => (
            None,
            Some(format!("Weekly stats could not be loaded: {error}")),
        ),
    };

    Ok(DashboardStats {
        today,
        week,
        week_error,
    })
}

impl AppModel {
    fn refresh_task(&mut self) -> Task<cosmic::Action<Message>> {
        self.connection = ConnectionConfig::load();
        self.status = Status::Loading;
        let connection = self.connection.clone();
        cosmic::task::future(async move { Message::Loaded(fetch_dashboard(connection).await) })
    }

    fn panel_content(&self, horizontal: bool) -> Element<'_, Message> {
        let glyph = icon::from_name("clock-symbolic").size(16);
        let value = self.core.applet.text(self.status.panel_text());
        if horizontal {
            row![glyph, value]
                .spacing(6)
                .align_y(Alignment::Center)
                .into()
        } else {
            column![glyph, value]
                .spacing(3)
                .align_x(Alignment::Center)
                .into()
        }
    }

    fn breakdown_section<'a>(label: &'a str, items: &'a [Breakdown]) -> Element<'a, Message> {
        let mut list = column![text::caption(label)].spacing(7).width(Length::Fill);
        if items.is_empty() {
            list = list.push(text::body("No activity in this range"));
        } else {
            for item in items.iter().take(5) {
                list = list.push(
                    row![
                        text::body(&item.name),
                        space::horizontal().width(Length::Fill),
                        text::caption(format!("{} · {:.0}%", item.text, item.percent)),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                );
            }
        }

        container(list)
            .padding(12)
            .width(Length::Fill)
            .class(cosmic::theme::Container::Card)
            .into()
    }
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(core: cosmic::Core, _flags: Self::Flags) -> (Self, Task<cosmic::Action<Message>>) {
        let connection = ConnectionConfig::load();
        let mut app = Self {
            core,
            popup: None,
            status: Status::Loading,
            connection,
        };
        let task = app.refresh_task();
        (app, task)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        cosmic::iced::time::every(REFRESH_INTERVAL).map(|_| Message::Refresh)
    }

    fn view(&self) -> Element<'_, Message> {
        let horizontal = matches!(
            self.core.applet.anchor,
            PanelAnchor::Top | PanelAnchor::Bottom
        );
        button::custom(self.panel_content(horizontal))
            .padding(if horizontal {
                [0, self.core.applet.suggested_padding(true).0]
            } else {
                [self.core.applet.suggested_padding(true).0, 0]
            })
            .on_press_down(Message::TogglePopup)
            .class(cosmic::theme::Button::AppletIcon)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let (headline, detail) = match &self.status {
            Status::Loading => ("Refreshing…".to_owned(), "Contacting Hackatime".to_owned()),
            Status::Ready(stats) => {
                let suffix = if stats.today.total_seconds < 60.0 && stats.today.total_seconds > 0.0
                {
                    " · less than a minute"
                } else {
                    ""
                };
                let freshness =
                    stats
                        .today
                        .updated_at
                        .elapsed()
                        .map_or("just now".to_owned(), |elapsed| {
                            if elapsed.as_secs() < 60 {
                                "just now".to_owned()
                            } else {
                                format!("{}m ago", elapsed.as_secs() / 60)
                            }
                        });
                (
                    stats.today.text.clone(),
                    format!("Coded today{suffix} · updated {freshness}"),
                )
            }
            Status::Error(error) => (error.panel_text().to_owned(), error.detail()),
        };

        let header = row![
            column![
                text::title3("HackaStats"),
                text::caption("Today's coding time")
            ]
            .spacing(2),
            space::horizontal().width(Length::Fill),
            button::icon(icon::from_name("view-refresh-symbolic"))
                .padding(8)
                .on_press(Message::Refresh),
        ]
        .align_y(Alignment::Center);

        let summary = container(
            column![
                text::caption("TODAY"),
                text::title1(headline),
                text::body(detail)
            ]
            .spacing(4),
        )
        .padding(14)
        .width(Length::Fill)
        .class(cosmic::theme::Container::Card);

        let mut dashboard = column![summary].spacing(10).width(Length::Fill);
        if let Status::Ready(stats) = &self.status {
            if let Some(week) = &stats.week {
                let totals = row![
                    container(
                        column![
                            text::caption("LAST 7 DAYS"),
                            text::title3(&week.human_readable_total)
                        ]
                        .spacing(4),
                    )
                    .padding(12)
                    .width(Length::FillPortion(1))
                    .class(cosmic::theme::Container::Card),
                    container(
                        column![
                            text::caption("DAILY AVERAGE"),
                            text::title3(&week.human_readable_daily_average)
                        ]
                        .spacing(4),
                    )
                    .padding(12)
                    .width(Length::FillPortion(1))
                    .class(cosmic::theme::Container::Card),
                ]
                .spacing(10);
                dashboard = dashboard
                    .push(totals)
                    .push(Self::breakdown_section("TOP LANGUAGES", &week.languages))
                    .push(Self::breakdown_section("TOP PROJECTS", &week.projects));
            } else if let Some(error) = &stats.week_error {
                dashboard = dashboard.push(
                    container(column![text::caption("LAST 7 DAYS"), text::body(error)].spacing(4))
                        .padding(12)
                        .width(Length::Fill)
                        .class(cosmic::theme::Container::Card),
                );
            }
        }

        dashboard = dashboard.push(
            button::text("Open full Hackatime dashboard")
                .leading_icon(icon::from_name("external-link-symbolic"))
                .on_press(Message::OpenDashboard),
        );

        let source = column![
            text::caption("CONNECTION"),
            text::body(format!("Settings: {}", self.connection.source)),
            text::caption(format!(
                "Refreshes every {} seconds",
                REFRESH_INTERVAL.as_secs()
            )),
        ]
        .spacing(4);

        let content = column![
            header,
            scrollable(dashboard).height(Length::Fill),
            divider::horizontal::default(),
            source,
            text::caption("The API key is read locally and is never displayed."),
        ]
        .spacing(12)
        .padding(18)
        .width(Length::Fill);

        self.core
            .applet
            .popup_container(
                container(content)
                    .width(Length::Fixed(480.0))
                    .height(Length::Fixed(620.0)),
            )
            .into()
    }

    fn update(&mut self, message: Message) -> Task<cosmic::Action<Message>> {
        match message {
            Message::Refresh => self.refresh_task(),
            Message::Loaded(result) => {
                self.status = match result {
                    Ok(stats) => Status::Ready(stats),
                    Err(error) => Status::Error(error),
                };
                Task::none()
            }
            Message::OpenDashboard => {
                let _ = std::process::Command::new("xdg-open")
                    .arg(self.connection.dashboard_url())
                    .spawn();
                Task::none()
            }
            Message::TogglePopup => {
                if let Some(id) = self.popup.take() {
                    cosmic::iced::platform_specific::shell::wayland::commands::popup::destroy_popup(
                        id,
                    )
                } else {
                    let id = Id::unique();
                    self.popup = Some(id);
                    let mut settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        id,
                        None,
                        None,
                        None,
                    );
                    settings.positioner.size_limits = Limits::NONE
                        .min_width(440.0)
                        .max_width(520.0)
                        .min_height(420.0)
                        .max_height(700.0);
                    cosmic::iced::platform_specific::shell::wayland::commands::popup::get_popup(
                        settings,
                    )
                }
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wakatime_settings() {
        let contents = "[settings]\napi_url = https://example.com/api/v1/\napi_key = x\n";
        let mut config = ConnectionConfig {
            base_url: DEFAULT_API_URL.to_owned(),
            api_key: String::new(),
            source: String::new(),
        };
        apply_wakatime_config(contents, &mut config);
        assert_eq!(config.base_url, "https://example.com/api/v1");
        assert_eq!(config.api_key, "x");
    }

    #[test]
    fn parses_status_response() {
        let payload: StatusResponse = serde_json::from_str(
            r#"{"data":{"grand_total":{"text":"2 hrs 14 mins","total_seconds":8040}}}"#,
        )
        .expect("valid response");
        assert_eq!(payload.data.grand_total.text, "2 hrs 14 mins");
        assert_eq!(payload.data.grand_total.total_seconds, 8040.0);
    }

    #[test]
    fn parses_week_response() {
        let payload: WeekResponse = serde_json::from_str(
            r#"{"data":{"human_readable_total":"12 hrs 5 mins","human_readable_daily_average":"1 hr 43 mins","languages":[{"name":"Rust","text":"8 hrs","percent":66.1}],"projects":[{"name":"hackastats","text":"4 hrs","percent":33.0}]}}"#,
        )
        .expect("valid response");
        assert_eq!(payload.data.human_readable_total, "12 hrs 5 mins");
        assert_eq!(payload.data.languages[0].name, "Rust");
        assert_eq!(payload.data.projects[0].name, "hackastats");
    }

    #[test]
    fn derives_dashboard_url_from_api_url() {
        let config = ConnectionConfig {
            base_url: "https://hackatime.example/api/hackatime/v1".to_owned(),
            api_key: String::new(),
            source: String::new(),
        };
        assert_eq!(
            config.dashboard_url(),
            "https://hackatime.example/dashboard"
        );
    }
}
