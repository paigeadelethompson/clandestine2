#[cfg(test)]
mod tests {
    use crate::ts6::TS6Message;
    use crate::ts6::parser::parse_message;

    #[test]
    fn test_parse_basic_message() {
        let msg = parse_message("NICK test 1 :Real Name");
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert_eq!(msg.command, "NICK");
        assert_eq!(msg.params, vec!["test", "1", "Real Name"]);
        assert!(msg.source.is_none());
    }

    #[test]
    fn test_parse_message_with_source() {
        let msg = parse_message(":server.test NICK test 1 :Real Name");
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert_eq!(msg.source, Some("server.test".to_string()));
        assert_eq!(msg.command, "NICK");
        assert_eq!(msg.params, vec!["test", "1", "Real Name"]);
    }

    #[test]
    fn test_parse_message_with_tags() {
        let msg = parse_message("@time=12345;id=abc :nick!user@host PRIVMSG #channel :Hello");
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert_eq!(msg.tags.get("time"), Some(&"12345".to_string()));
        assert_eq!(msg.tags.get("id"), Some(&"abc".to_string()));
        assert_eq!(msg.source, Some("nick!user@host".to_string()));
        assert_eq!(msg.command, "PRIVMSG");
        assert_eq!(msg.params, vec!["#channel", "Hello"]);
    }

    #[test]
    fn test_message_to_string() {
        let mut msg = TS6Message::new("PRIVMSG".to_string(), vec!["#test".to_string(), "Hello".to_string()]);
        msg.source = Some("nick!user@host".to_string());
        assert_eq!(msg.to_string(), ":nick!user@host PRIVMSG #test :Hello");
    }
} 