// Copyright (c) 2025, vistone
// All rights reserved.

#[cfg(test)]
mod tests {
    use netconnpool::*;

    #[test]
    fn test_stats_collector() {
        let collector = StatsCollector::new();
        
        collector.increment_total_connections_created();
        collector.increment_total_connections_created();
        
        let stats = collector.get_stats();
        assert_eq!(stats.total_connections_created, 2);
        assert_eq!(stats.current_connections, 2);
    }

    #[test]
    fn test_stats_increment() {
        let collector = StatsCollector::new();
        
        collector.increment_successful_gets();
        collector.increment_failed_gets();
        collector.increment_connection_errors();
        
        let stats = collector.get_stats();
        assert_eq!(stats.successful_gets, 1);
        assert_eq!(stats.failed_gets, 1);
        assert_eq!(stats.connection_errors, 1);
    }
}
