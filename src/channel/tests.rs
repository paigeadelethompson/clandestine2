use super::*;
use chrono::Utc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_creation() {
        let channel = Channel::new("#test".to_string());
        assert_eq!(channel.name, "#test");
        assert!(channel.topic.is_none());
        assert!(channel.members.is_empty());
        assert!(channel.modes.contains(&'n')); // Default mode +n
        assert!(channel.modes.contains(&'t')); // Default mode +t
    }

    #[test]
    fn test_member_management() {
        let mut channel = Channel::new("#test".to_string());
        
        // Test adding members
        channel.add_member(1);
        channel.add_member(2);
        assert_eq!(channel.get_members().len(), 2);
        assert!(channel.get_members().contains(&1));
        assert!(channel.get_members().contains(&2));

        // Test removing members
        channel.remove_member(1);
        assert_eq!(channel.get_members().len(), 1);
        assert!(!channel.get_members().contains(&1));
        assert!(channel.get_members().contains(&2));
    }

    #[test]
    fn test_topic_management() {
        let mut channel = Channel::new("#test".to_string());
        
        // Test setting topic
        channel.set_topic("Test Topic".to_string(), "tester".to_string());
        assert_eq!(channel.get_topic(), Some("Test Topic".to_string()));
        
        let (topic, setter, _time) = channel.get_topic_details();
        assert_eq!(topic, Some("Test Topic".to_string()));
        assert_eq!(setter, Some("tester".to_string()));
    }

    #[test]
    fn test_mode_management() {
        let mut channel = Channel::new("#test".to_string());
        
        // Test setting modes
        channel.set_mode('i', None, true);
        assert!(channel.has_mode('i', None));
        
        // Test setting mode with parameter
        channel.set_mode('k', Some("password".to_string()), true);
        assert!(channel.has_mode('k', Some("password")));
        
        // Test removing modes
        channel.set_mode('i', None, false);
        assert!(!channel.has_mode('i', None));
    }

    #[test]
    fn test_ban_management() {
        let mut channel = Channel::new("#test".to_string());
        
        // Test adding ban
        channel.add_ban("*!*@badhost.com".to_string(), "tester".to_string());
        assert_eq!(channel.get_bans().len(), 1);
        assert_eq!(channel.get_bans()[0].mask, "*!*@badhost.com");
        
        // Test removing ban
        channel.remove_ban("*!*@badhost.com");
        assert_eq!(channel.get_bans().len(), 0);
    }
} 