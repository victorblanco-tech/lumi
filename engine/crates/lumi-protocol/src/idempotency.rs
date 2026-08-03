use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fmt::{Display, Formatter};

/// Indicates whether a mutating command may be applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandDisposition {
    FirstSeen,
    Duplicate,
}

/// Bounded cache for command IDs that makes mutating command handling
/// idempotent without unbounded memory growth.
#[derive(Debug)]
pub struct CommandIdCache {
    capacity: usize,
    insertion_order: VecDeque<String>,
    identifiers: HashSet<String>,
}

impl CommandIdCache {
    pub fn new(capacity: usize) -> Result<Self, InvalidCacheCapacity> {
        if capacity == 0 {
            return Err(InvalidCacheCapacity);
        }

        Ok(Self {
            capacity,
            insertion_order: VecDeque::with_capacity(capacity),
            identifiers: HashSet::with_capacity(capacity),
        })
    }

    /// Records a command ID and returns whether it is safe to apply.
    pub fn observe(&mut self, message_id: &str) -> CommandDisposition {
        if self.identifiers.contains(message_id) {
            return CommandDisposition::Duplicate;
        }

        if self.identifiers.len() == self.capacity
            && let Some(expired) = self.insertion_order.pop_front()
        {
            self.identifiers.remove(&expired);
        }

        let owned_identifier = message_id.to_owned();
        self.insertion_order.push_back(owned_identifier.clone());
        self.identifiers.insert(owned_identifier);
        CommandDisposition::FirstSeen
    }

    #[must_use]
    pub fn contains(&self, message_id: &str) -> bool {
        self.identifiers.contains(message_id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidCacheCapacity;

impl Display for InvalidCacheCapacity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("command ID cache capacity must be greater than zero")
    }
}

impl Error for InvalidCacheCapacity {}

#[cfg(test)]
mod tests {
    use super::{CommandDisposition, CommandIdCache, InvalidCacheCapacity};

    #[test]
    fn duplicate_command_does_not_apply_twice() -> Result<(), InvalidCacheCapacity> {
        let mut cache = CommandIdCache::new(16)?;
        let mut applied_mutations = 0;

        for message_id in ["command-1", "command-1"] {
            if cache.observe(message_id) == CommandDisposition::FirstSeen {
                applied_mutations += 1;
            }
        }

        assert_eq!(applied_mutations, 1);
        Ok(())
    }

    #[test]
    fn cache_evicts_oldest_identifier_at_capacity() -> Result<(), InvalidCacheCapacity> {
        let mut cache = CommandIdCache::new(2)?;

        assert_eq!(cache.observe("one"), CommandDisposition::FirstSeen);
        assert_eq!(cache.observe("two"), CommandDisposition::FirstSeen);
        assert_eq!(cache.observe("three"), CommandDisposition::FirstSeen);
        assert_eq!(cache.observe("one"), CommandDisposition::FirstSeen);
        Ok(())
    }
}
