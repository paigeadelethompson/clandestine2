#[cfg(test)]
mod tests {
    use crate::ts6::TS6Message;
    use crate::ts6::parser::parse_message;

    #[test]
    fn test_parse_basic_message() {
        let msg = parse_message("NICK test 1 :Real Name");
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.command, "NICK");
        assert_eq!(msg.params, vec!["test", "1", "Real Name"]);
    }

    #[test]
    fn test_parse_message_with_source() {
        let msg = parse_message(":server1 PING :server2");
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.source.unwrap(), "server1");
        assert_eq!(msg.command, "PING");
        assert_eq!(msg.params, vec!["server2"]);
    }

    #[test]
    fn test_parse_message_with_tags() {
        let msg = parse_message("@time=12345;id=abc :nick!user@host PRIVMSG #channel :Hello");
        assert!(msg.is_ok());
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

    #[test]
    fn test_message_with_empty_param() {
        let msg = TS6Message::new("AWAY".to_string(), vec!["".to_string()]);
        assert_eq!(msg.to_string(), "AWAY :");
    }

    #[test]
    fn test_message_with_multiple_params() {
        let msg = TS6Message::new(
            "MODE".to_string(), 
            vec!["#channel".to_string(), "+o".to_string(), "user".to_string()]
        );
        assert_eq!(msg.to_string(), "MODE #channel +o :user");
    }

    #[test]
    fn test_message_with_spaces_in_middle() {
        let msg = TS6Message::new(
            "NOTICE".to_string(),
            vec!["#ops".to_string(), "Server notice".to_string(), "more text".to_string()]
        );
        assert_eq!(msg.to_string(), "NOTICE #ops Server notice :more text");
    }

    #[test]
    fn test_parse_complex_message() {
        let msg = parse_message(":nick!user@host PRIVMSG #channel :Hello there!");
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.source.unwrap(), "nick!user@host");
        assert_eq!(msg.command, "PRIVMSG");
        assert_eq!(msg.params, vec!["#channel", "Hello there!"]);
    }

    #[test]
    fn test_parse_numeric() {
        let msg = parse_message(":server 001 nick :Welcome to the network");
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.source.unwrap(), "server");
        assert_eq!(msg.command, "001");
        assert_eq!(msg.params, vec!["nick", "Welcome to the network"]);
    }

    #[test]
    fn test_parse_invalid_message() {
        assert!(parse_message("").is_err());
        assert!(parse_message(":").is_err());
        assert!(parse_message("COMMAND :").is_err());
    }

    #[test]
    fn test_parse_message() {
        let msg = parse_message(":source COMMAND param1 param2 :trailing");
        assert!(msg.is_ok());
        let msg = msg.unwrap();
        assert_eq!(msg.source.unwrap(), "source");
        assert_eq!(msg.command, "COMMAND");
        assert_eq!(msg.params, vec!["param1", "param2", "trailing"]);
    }
} 