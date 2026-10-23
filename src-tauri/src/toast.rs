//! Windows toasts built by Marshell, so one banner carries the session's `marshell://` link everywhere a click
//! can land: the toast body, its button and, through `launch`, the notification centre entry that outlives the app.
//!
//! The XML skeleton holds no text of the person's. Titles, bodies and links are set through `SetInnerText` and
//! `SetAttribute`, so a title like `</text><action …>` stays text.

#![cfg(windows)]

use crate::{link, notify};
use marshell_core::notify::{Banner, BannerAction};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::Mutex;
use tauri::Manager;
use windows::{
    Data::Xml::Dom::XmlDocument,
    Foundation::TypedEventHandler,
    UI::Notifications::{ToastActivatedEventArgs, ToastNotification, ToastNotificationManager},
    core::{IInspectable, Interface, h},
};

/// How many shown toasts are kept alive. An `Activated` handler lives only as long as its toast object.
const KEPT: usize = 32;

/// The id Windows files a toast under before the app has its own: PowerShell's, which the plugin uses for an
/// unbundled `target\debug` or `target\release` build, so a dev banner shows at all.
const POWERSHELL_AUMID: &str = r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe";

/// The toast's app id, following the plugin: PowerShell's from a `target\debug` or `target\release` folder, the
/// bundle identifier from an install.
pub fn aumid(exe_dir: &Path, identifier: &str) -> String {
    if exe_dir.ends_with(r"target\debug") || exe_dir.ends_with(r"target\release") {
        POWERSHELL_AUMID.to_string()
    } else {
        identifier.to_string()
    }
}

/// The running app's toast id.
pub fn app_aumid(app: &tauri::AppHandle) -> String {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    aumid(dir.as_deref().unwrap_or(Path::new("")), &app.config().identifier)
}

/// The toast for a banner. `protocol` is whether the link is registered for this exe: a click then runs the
/// link as a protocol launch, so it reaches Marshell even from the notification centre after Marshell closed.
/// Otherwise the click activates the running app in the foreground.
pub fn toast_xml(banner: &Banner, link: &str, protocol: bool) -> windows::core::Result<XmlDocument> {
    let kind = if protocol { h!("protocol") } else { h!("foreground") };
    let doc = XmlDocument::new()?;

    let toast = doc.CreateElement(h!("toast"))?;
    toast.SetAttribute(h!("launch"), &link.into())?;
    toast.SetAttribute(h!("activationType"), kind)?;

    let visual = doc.CreateElement(h!("visual"))?;
    let binding = doc.CreateElement(h!("binding"))?;
    binding.SetAttribute(h!("template"), h!("ToastGeneric"))?;
    let title = doc.CreateElement(h!("text"))?;
    title.SetInnerText(&(&banner.title).into())?;
    let body = doc.CreateElement(h!("text"))?;
    body.SetInnerText(&notify::body_of(banner).into())?;
    binding.AppendChild(&title)?;
    binding.AppendChild(&body)?;
    visual.AppendChild(&binding)?;
    toast.AppendChild(&visual)?;

    // The shell plays the sound itself (attention::play_sound), so a banner is always silent.
    let audio = doc.CreateElement(h!("audio"))?;
    audio.SetAttribute(h!("silent"), h!("true"))?;
    toast.AppendChild(&audio)?;

    let actions = doc.CreateElement(h!("actions"))?;
    let action = doc.CreateElement(h!("action"))?;
    let content = match banner.action {
        BannerAction::Open => h!("Open"),
        BannerAction::Review => h!("Review"),
    };
    action.SetAttribute(h!("content"), content)?;
    action.SetAttribute(h!("arguments"), &link.into())?;
    action.SetAttribute(h!("activationType"), kind)?;
    actions.AppendChild(&action)?;
    toast.AppendChild(&actions)?;

    doc.AppendChild(&toast)?;
    Ok(doc)
}

/// Shows a banner. A click that Windows delivers to this process (foreground activation) is read by
/// `link::parse` and opened by `link::open`, like a second launch's link.
pub fn show(app: &tauri::AppHandle, banner: &Banner) -> windows::core::Result<()> {
    let link = link::link_for(banner.tab_id.as_deref());
    let doc = toast_xml(banner, &link, link::registered_here())?;
    let toast = ToastNotification::CreateToastNotification(&doc)?;

    let handle = app.clone();
    toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
        move |_, args| {
            let arguments = args
                .as_ref()
                .and_then(|a| a.cast::<ToastActivatedEventArgs>().ok())
                .and_then(|a| a.Arguments().ok())
                .map(|a| a.to_string());
            eprintln!("Marshell: banner clicked, link {arguments:?}");
            link::open(&handle, arguments.as_deref().and_then(link::parse));
            Ok(())
        },
    ))?;

    ToastNotificationManager::CreateToastNotifierWithId(&app_aumid(app).as_str().into())?.Show(&toast)?;

    // Keep the toast objects, or their handlers go with them.
    if app.try_state::<Mutex<VecDeque<ToastNotification>>>().is_none() {
        app.manage(Mutex::new(VecDeque::<ToastNotification>::new()));
    }
    if let Some(kept) = app.try_state::<Mutex<VecDeque<ToastNotification>>>() {
        let mut kept = kept.lock().unwrap_or_else(|e| e.into_inner());
        kept.push_back(toast);
        while kept.len() > KEPT {
            kept.pop_front();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Data::Xml::Dom::XmlElement;

    fn banner(title: &str, action: BannerAction) -> Banner {
        Banner {
            title: title.into(),
            body: "Wants to run `npm test`".into(),
            meta: "Session 3 · api".into(),
            action,
            tab_id: Some("t3".into()),
        }
    }

    fn xml(banner: &Banner, protocol: bool) -> String {
        toast_xml(banner, "marshell://focus/t3", protocol)
            .unwrap()
            .GetXml()
            .unwrap()
            .to_string()
    }

    #[test]
    fn the_toast_carries_the_link_everywhere() {
        let xml = xml(&banner("api needs you", BannerAction::Open), true);
        println!("{xml}");
        assert!(xml.contains(r#"launch="marshell://focus/t3""#), "{xml}");
        assert!(
            xml.starts_with(r#"<toast launch="marshell://focus/t3" activationType="protocol">"#),
            "{xml}"
        );
        assert_eq!(xml.matches("<action ").count(), 1, "{xml}");
        assert!(
            xml.contains(r#"<action content="Open" arguments="marshell://focus/t3" activationType="protocol"/>"#),
            "{xml}"
        );
        assert!(xml.contains(r#"<audio silent="true"/>"#), "{xml}");
    }

    #[test]
    fn foreground_when_the_link_is_not_registered() {
        let xml = xml(&banner("api needs you", BannerAction::Open), false);
        assert_eq!(xml.matches(r#"activationType="foreground""#).count(), 2, "{xml}");
        assert!(!xml.contains("protocol"), "{xml}");
    }

    #[test]
    fn review_is_the_button_for_a_risky_request() {
        let xml = xml(&banner("api needs you", BannerAction::Review), true);
        assert!(xml.contains(r#"content="Review""#), "{xml}");
        assert!(!xml.contains(r#"content="Open""#), "{xml}");
    }

    #[test]
    fn text_stays_text() {
        let title = r#"x</text><action activationType="protocol" arguments="https://example.com"/>"#;
        let b = banner(title, BannerAction::Open);
        let doc = toast_xml(&b, "marshell://focus/t3", true).unwrap();
        assert_eq!(doc.GetElementsByTagName(h!("action")).unwrap().Length().unwrap(), 1);
        let texts = doc.GetElementsByTagName(h!("text")).unwrap();
        assert_eq!(texts.Length().unwrap(), 2);
        let inner = |i| {
            texts
                .Item(i)
                .unwrap()
                .cast::<XmlElement>()
                .unwrap()
                .InnerText()
                .unwrap()
                .to_string()
        };
        assert_eq!(inner(0), title);
        assert_eq!(inner(1), notify::body_of(&b));
        assert_eq!(inner(1).lines().count(), 2);
    }

    #[test]
    fn aumid_follows_the_plugin() {
        assert_eq!(
            aumid(Path::new(r"C:\r\target\debug"), "dev.marshell.app"),
            POWERSHELL_AUMID
        );
        assert_eq!(
            aumid(Path::new(r"C:\r\target\release"), "dev.marshell.app"),
            POWERSHELL_AUMID
        );
        assert_eq!(
            aumid(Path::new(r"C:\Program Files\Marshell"), "dev.marshell.app"),
            "dev.marshell.app"
        );
    }
}
