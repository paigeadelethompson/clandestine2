use super::*;
use chrono::Utc;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_basic_channel_operations() {
        let mut channel = Channel::new("#test".to_string());
        
        // Test initial state
        assert_eq!(channel.name, "#test");
        assert!(channel.members.is_empty());
        assert!(channel.modes.is_empty());
        assert!(channel.topic.is_none());
    }

    #[test]
    fn test_member_management() {
        let mut channel = Channel::new("#test".to_string());
        
        // Add members
        channel.add_member(1);
        channel.add_member(2);
        assert_eq!(channel.members.len(), 2);
        assert!(channel.has_member(1));
        assert!(channel.has_member(2));
        
        // Remove member
        channel.remove_member(1);
        assert!(!channel.has_member(1));
        assert!(channel.has_member(2));
        assert_eq!(channel.members.len(), 1);
    }

    #[test]
    fn test_channel_modes() {
        let mut channel = Channel::new("#test".to_string());
        
        // Set modes
        channel.set_mode('n', None, true);
        channel.set_mode('t', None, true);
        assert!(channel.has_mode('n', None));
        assert!(channel.has_mode('t', None));
        
        // Remove mode
        channel.set_mode('n', None, false);
        assert!(!channel.has_mode('n', None));
        assert!(channel.has_mode('t', None));
    }

    #[test]
    fn test_channel_topic() {
        let mut channel = Channel::new("#test".to_string());
        let now = Utc::now();
        
        // Set topic
        channel.set_topic("Test Topic".to_string(), "nick!user@host".to_string());
        assert_eq!(channel.topic.as_ref().unwrap(), "Test Topic");
        assert_eq!(channel.topic_setter.as_ref().unwrap(), "nick!user@host");
        assert!(channel.topic_time > now);
        
        // Clear topic
        channel.set_topic("".to_string(), "another@user".to_string());
        assert!(channel.topic.as_ref().unwrap().is_empty());
        assert_eq!(channel.topic_setter.as_ref().unwrap(), "another@user");
    }

    #[test]
    fn test_channel_bans() {
        let mut channel = Channel::new("#test".to_string());
        
        // Add ban
        channel.add_ban("*!*@banned.com".to_string(), "op!user@host".to_string());
        assert!(channel.is_banned("nick!user@banned.com"));
        assert!(!channel.is_banned("nick!user@allowed.com"));
        
        // Remove ban
        channel.remove_ban("*!*@banned.com");
        assert!(!channel.is_banned("nick!user@banned.com"));
    }

    #[test]
    fn test_channel_operators() {
        let mut channel = Channel::new("#test".to_string());
        
        // Add operator
        channel.add_member(1);
        channel.set_operator(1, true);
        assert!(channel.is_operator(1));
        
        // Remove operator
        channel.set_operator(1, false);
        assert!(!channel.is_operator(1));
        
        // Non-member can't be operator
        channel.set_operator(2, true);
        assert!(!channel.is_operator(2));
    }

    #[test]
    fn test_channel_voices() {
        let mut channel = Channel::new("#test".to_string());
        
        // Add voice
        channel.add_member(1);
        channel.set_voice(1, true);
        assert!(channel.is_voiced(1));
        
        // Remove voice
        channel.set_voice(1, false);
        assert!(!channel.is_voiced(1));
    }
} 