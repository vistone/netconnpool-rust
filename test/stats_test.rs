// Copyright (c) 2025, vistone
// All rights reserved.

#[cfg(test)]
mod tests {
    use netconnpool::*;

    #[test]
    fn test_stats_collector() {
        let collector = StatsCollector::new();
        
        collector.IncrementTotalConnectionsCreated();
        collector.IncrementTotalConnectionsCreated();
        
        let stats = collector.GetStats();
        assert_eq!(stats.TotalConnectionsCreated, 2);
        assert_eq!(stats.CurrentConnections, 2);
    }

    #[test]
    fn test_stats_increment() {
        let collector = StatsCollector::new();
        
        collector.IncrementSuccessfulGets();
        collector.IncrementFailedGets();
        collector.IncrementConnectionErrors();
        
        let stats = collector.GetStats();
        assert_eq!(stats.SuccessfulGets, 1);
        assert_eq!(stats.FailedGets, 1);
        assert_eq!(stats.ConnectionErrors, 1);
    }
}
