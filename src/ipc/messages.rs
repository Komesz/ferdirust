/// Message types for sidebar <-> service communication

#[derive(Debug)]
#[allow(dead_code)]
pub enum SidebarMessage {
    SwitchService { id: String },
    ReloadService { id: String },
    UpdateBadge { id: String, direct: u32, indirect: u32 },
}

impl SidebarMessage {
    #[allow(dead_code)]
    pub fn parse(msg: &str) -> Option<Self> {
        let parts: Vec<&str> = msg.splitn(2, ':').collect();
        match parts.first() {
            Some(&"switch") => parts.get(1).map(|id| SidebarMessage::SwitchService {
                id: id.to_string(),
            }),
            Some(&"reload") => parts.get(1).map(|id| SidebarMessage::ReloadService {
                id: id.to_string(),
            }),
            Some(&"badge") => {
                if let Some(payload) = parts.get(1) {
                    let badge_parts: Vec<&str> = payload.splitn(3, ':').collect();
                    if badge_parts.len() == 3 {
                        if let (Ok(direct), Ok(indirect)) =
                            (badge_parts[1].parse(), badge_parts[2].parse())
                        {
                            return Some(SidebarMessage::UpdateBadge {
                                id: badge_parts[0].to_string(),
                                direct,
                                indirect,
                            });
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }
}
