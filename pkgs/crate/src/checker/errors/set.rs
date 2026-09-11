//! Common builtin type sets use an inline bitset. Other singletons stay inline,
//! and larger mixed unions share a copy-on-write hash set. Mutation compacts sets
//! back to their canonical representation so equality remains independent of
//! construction order and representation history.
use super::*;

// This is a storage optimization, not a closed list of supported types. New
// builtin names and declaration identities continue to use the hash-set path.
macro_rules! builtin_bits {
    ($($index:literal => $name:literal),* $(,)?) => {
        static BUILTINS: &[EscErrorType] = &[$(EscErrorType::builtin($name)),*];
        fn builtin_bit(value: &EscErrorType) -> u64 {
            let EscErrorType::Buildin(name) = value else { return 0; };
            match name.as_str() { $($name => 1 << $index,)* _ => 0 }
        }
    };
}
builtin_bits! {
    0 => "unknown", 1 => "undefined", 2 => "null", 3 => "boolean",
    4 => "number", 5 => "string", 6 => "bigint", 7 => "symbol",
    8 => "Function", 9 => "object", 10 => "Array", 11 => "Promise",
    12 => "Generator", 13 => "AsyncGenerator", 14 => "Error", 15 => "TypeError",
    16 => "RangeError", 17 => "ReferenceError", 18 => "SyntaxError", 19 => "URIError",
    20 => "EvalError", 21 => "AggregateError", 22 => "DataView", 23 => "Uint8Array",
    24 => "ArrayBuffer", 25 => "SharedArrayBuffer",
}

fn next_builtin(bits: &mut u64) -> Option<&'static EscErrorType> {
    if *bits == 0 {
        return None;
    }
    let value = &BUILTINS[bits.trailing_zeros() as usize];
    *bits &= *bits - 1;
    Some(value)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum Types {
    #[default]
    Empty,
    One(EscErrorType),
    Builtins(u64),
    Many(Arc<HashSet<EscErrorType>>),
}

impl Types {
    fn from_bits(bits: u64) -> Self {
        if bits == 0 {
            Self::Empty
        } else {
            Self::Builtins(bits)
        }
    }
    fn singleton(value: EscErrorType) -> Self {
        let bit = builtin_bit(&value);
        if bit == 0 {
            Self::One(value)
        } else {
            Self::Builtins(bit)
        }
    }
    fn bits(&self) -> Option<u64> {
        match self {
            Self::Empty => Some(0),
            Self::One(_) => None,
            Self::Builtins(bits) => Some(*bits),
            Self::Many(_) => None,
        }
    }
    pub(super) fn new() -> Self {
        Self::Empty
    }
    pub(super) fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
    pub(super) fn plain_primitive(&self) -> bool {
        // Slots 1..=5 are undefined, null, boolean, number, and string.
        // Other singleton/fallback types cannot be in this closed subset.
        match self {
            Self::Empty => true,
            Self::Builtins(bits) => bits & !0b11_1110 == 0,
            _ => false,
        }
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Builtins(bits) => bits.count_ones() as usize,
            Self::Many(set) => set.len(),
        }
    }
    pub(super) fn contains(&self, value: &EscErrorType) -> bool {
        match self {
            Self::Empty => false,
            Self::One(one) => one == value,
            Self::Builtins(bits) => bits & builtin_bit(value) != 0,
            Self::Many(set) => set.contains(value),
        }
    }
    pub(super) fn insert(&mut self, value: EscErrorType) -> bool {
        match self {
            Self::Empty => {
                *self = Self::singleton(value);
                true
            }
            Self::One(one) if *one == value => false,
            Self::One(_) => {
                let Self::One(one) = std::mem::take(self) else {
                    unreachable!()
                };
                *self = Self::Many(Arc::new(HashSet::from([one, value])));
                true
            }
            Self::Builtins(bits) => {
                let bit = builtin_bit(&value);
                if bit != 0 {
                    let changed = *bits & bit == 0;
                    *bits |= bit;
                    changed
                } else {
                    let mut set = self.drain().collect::<HashSet<_>>();
                    set.insert(value);
                    *self = Self::Many(Arc::new(set));
                    true
                }
            }
            Self::Many(set) => {
                if set.contains(&value) {
                    false
                } else {
                    Arc::make_mut(set).insert(value)
                }
            }
        }
    }
    fn compact(&mut self) {
        match self {
            Self::Builtins(bits) => *self = Self::from_bits(*bits),
            Self::Many(set) if set.len() < 2 => {
                *self = set
                    .iter()
                    .next()
                    .cloned()
                    .map_or(Self::Empty, Self::singleton);
            }
            Self::Many(set) => {
                let bits = set.iter().try_fold(0, |bits, value| {
                    let bit = builtin_bit(value);
                    (bit != 0).then_some(bits | bit)
                });
                if let Some(bits) = bits {
                    *self = Self::from_bits(bits);
                }
            }
            _ => (),
        }
    }
    pub(super) fn remove(&mut self, value: &EscErrorType) -> bool {
        let removed = match self {
            Self::Empty => false,
            Self::One(one) => {
                if one != value {
                    return false;
                }
                *self = Self::Empty;
                true
            }
            Self::Builtins(bits) => {
                let bit = builtin_bit(value);
                let removed = *bits & bit != 0;
                *bits &= !bit;
                removed
            }
            Self::Many(set) => {
                if !set.contains(value) {
                    false
                } else {
                    Arc::make_mut(set).remove(value)
                }
            }
        };
        if removed {
            self.compact();
        }
        removed
    }
    pub(super) fn retain(&mut self, mut keep: impl FnMut(&EscErrorType) -> bool) {
        match self {
            Self::Empty => (),
            Self::One(one) => {
                if !keep(one) {
                    *self = Self::Empty;
                }
            }
            Self::Builtins(bits) => {
                let mut remaining = *bits;
                while remaining != 0 {
                    let index = remaining.trailing_zeros() as usize;
                    let bit = 1 << index;
                    remaining &= remaining - 1;
                    if !keep(&BUILTINS[index]) {
                        *bits &= !bit;
                    }
                }
            }
            Self::Many(set) => {
                let removed = set
                    .iter()
                    .filter(|value| !keep(value))
                    .cloned()
                    .collect::<Vec<_>>();
                if removed.is_empty() {
                    return;
                }
                let set = Arc::make_mut(set);
                for value in removed {
                    set.remove(&value);
                }
            }
        }
        self.compact();
    }
    pub(super) fn iter(&self) -> Iter<'_> {
        match self {
            Self::Empty => Iter::Empty,
            Self::One(one) => Iter::One(Some(one)),
            Self::Builtins(bits) => Iter::Builtins(*bits),
            Self::Many(set) => Iter::Many(set.iter()),
        }
    }
    pub(super) fn drain(&mut self) -> IntoIter {
        std::mem::take(self).into_iter()
    }
    pub(super) fn join(&mut self, incoming: Self) {
        if incoming.is_empty() {
            return;
        }
        if let (Self::One(left), Self::One(right)) = (&self, &incoming)
            && left == right
        {
            return;
        }
        if let (Self::Many(target), Self::Many(source)) = (&self, &incoming)
            && Arc::ptr_eq(target, source)
        {
            return;
        }
        if self.is_empty() {
            *self = incoming;
        } else if let (Some(left), Some(right)) = (self.bits(), incoming.bits()) {
            *self = Self::from_bits(left | right);
        } else if let Self::Many(source) = incoming {
            self.extend(source.iter().cloned());
        } else {
            self.extend(incoming);
        }
    }
}

impl<const N: usize> From<[EscErrorType; N]> for Types {
    fn from(values: [EscErrorType; N]) -> Self {
        values.into_iter().collect()
    }
}
impl From<HashSet<EscErrorType>> for Types {
    fn from(values: HashSet<EscErrorType>) -> Self {
        if values.len() < 2 {
            values
                .into_iter()
                .next()
                .map_or(Self::Empty, Self::singleton)
        } else {
            let mut result = Self::Many(Arc::new(values));
            result.compact();
            result
        }
    }
}
impl FromIterator<EscErrorType> for Types {
    fn from_iter<T: IntoIterator<Item = EscErrorType>>(iter: T) -> Self {
        let mut set = Self::Empty;
        set.extend(iter);
        set
    }
}
impl Extend<EscErrorType> for Types {
    fn extend<T: IntoIterator<Item = EscErrorType>>(&mut self, iter: T) {
        for value in iter {
            self.insert(value);
        }
    }
}

pub(super) enum Iter<'a> {
    Empty,
    One(Option<&'a EscErrorType>),
    Builtins(u64),
    Many(std::collections::hash_set::Iter<'a, EscErrorType>),
}
impl<'a> Iterator for Iter<'a> {
    type Item = &'a EscErrorType;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::One(one) => one.take(),
            Self::Builtins(bits) => next_builtin(bits),
            Self::Many(iter) => iter.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match self {
            Self::Empty => 0,
            Self::One(one) => usize::from(one.is_some()),
            Self::Builtins(bits) => bits.count_ones() as usize,
            Self::Many(iter) => iter.len(),
        };
        (len, Some(len))
    }
}
impl ExactSizeIterator for Iter<'_> {}
impl<'a> IntoIterator for &'a Types {
    type Item = &'a EscErrorType;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub(super) enum IntoIter {
    Empty,
    One(Option<EscErrorType>),
    Builtins(u64),
    Many(std::collections::hash_set::IntoIter<EscErrorType>),
}
impl Iterator for IntoIter {
    type Item = EscErrorType;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::One(one) => one.take(),
            Self::Builtins(bits) => next_builtin(bits).cloned(),
            Self::Many(iter) => iter.next(),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match self {
            Self::Empty => 0,
            Self::One(one) => usize::from(one.is_some()),
            Self::Builtins(bits) => bits.count_ones() as usize,
            Self::Many(iter) => iter.len(),
        };
        (len, Some(len))
    }
}
impl ExactSizeIterator for IntoIter {}
impl IntoIterator for Types {
    type Item = EscErrorType;
    type IntoIter = IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Empty => IntoIter::Empty,
            Self::One(one) => IntoIter::One(Some(one)),
            Self::Builtins(bits) => IntoIter::Builtins(bits),
            Self::Many(set) => IntoIter::Many(Arc::unwrap_or_clone(set).into_iter()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_bits_round_trip_and_compact_after_fallback() {
        for (index, value) in BUILTINS.iter().enumerate() {
            assert_eq!(builtin_bit(value), 1u64 << index);
            assert_eq!(
                Types::from([value.clone()]).plain_primitive(),
                matches!(value, EscErrorType::Buildin(name) if matches!(name.as_str(), "undefined" | "null" | "boolean" | "number" | "string"))
            );
        }
        let all = BUILTINS.iter().cloned().collect::<Types>();
        assert!(matches!(all, Types::Builtins(_)));
        assert_eq!(
            all.iter().collect::<Vec<_>>(),
            BUILTINS.iter().collect::<Vec<_>>()
        );
        assert_eq!(all.clone().into_iter().collect::<Vec<_>>(), BUILTINS);
        let mut mixed = all.clone();
        let custom = EscErrorType::builtin("UnindexedBuiltin");
        mixed.insert(custom.clone());
        assert!(matches!(mixed, Types::Many(_)));
        assert!(mixed.remove(&custom));
        assert_eq!(mixed, all);
        let mut joined = Types::new();
        for value in BUILTINS.iter().rev() {
            joined.join(Types::from([value.clone()]));
        }
        assert_eq!(joined, all);
        mixed.retain(|value| value == &EscErrorType::UNKNOWN);
        assert_eq!(mixed, Types::from([EscErrorType::UNKNOWN]));
        for value in BUILTINS {
            assert!(joined.remove(value));
        }
        assert!(joined.is_empty());
    }

    #[test]
    fn inline_and_hashed_sets_match_through_mutations() {
        let values = [
            "number",
            "string",
            "unknown",
            "TypeError",
            "RangeError",
            "undefined",
            "UnindexedBuiltin",
        ]
        .map(EscErrorType::builtin);
        let mut small = Types::new();
        let mut reference = HashSet::new();
        let mut random = 17u64;
        for step in 0..2000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = values[(random >> 32) as usize % values.len()].clone();
            match step % 7 {
                0..=2 => assert_eq!(small.insert(value.clone()), reference.insert(value)),
                3 => assert_eq!(small.remove(&value), reference.remove(&value)),
                4 => {
                    small.retain(|ty| ty == &value);
                    reference.retain(|ty| ty == &value);
                }
                5 => {
                    small.join(Types::from([value.clone(), EscErrorType::UNKNOWN]));
                    reference.extend([value, EscErrorType::UNKNOWN]);
                }
                _ => {
                    assert_eq!(
                        small.drain().collect::<HashSet<_>>(),
                        std::mem::take(&mut reference)
                    );
                    assert!(small.is_empty());
                }
            }
            assert_eq!(small.iter().cloned().collect::<HashSet<_>>(), reference);
            assert_eq!(small.plain_primitive(), reference.iter().all(|value| matches!(value, EscErrorType::Buildin(name) if matches!(name.as_str(), "undefined" | "null" | "boolean" | "number" | "string"))));
            assert_eq!(small.len(), reference.len());
            assert_eq!(small.iter().len(), reference.len());
            assert_eq!(small, Types::from(reference.clone()));
            assert_eq!(small.clone().into_iter().collect::<HashSet<_>>(), reference);
            assert!(!matches!(&small, Types::Many(set) if set.len() < 2));
        }
    }

    #[test]
    fn singleton_unions_and_removals_remain_inline() {
        let number = EscErrorType::builtin("number");
        let string = EscErrorType::builtin("string");
        let mut set = Types::from([number.clone(), number.clone()]);
        assert!(matches!(set, Types::Builtins(_)));
        set.insert(string.clone());
        assert!(matches!(set, Types::Builtins(_)));
        assert!(set.remove(&number));
        assert_eq!(set, Types::from([string]));
        set.retain(|_| false);
        assert_eq!(set, Types::new());
    }

    #[test]
    fn shared_unions_preserve_branch_isolation_and_no_op_sharing() {
        let original = Types::from([
            EscErrorType::builtin("UnindexedBuiltin"),
            EscErrorType::builtin("string"),
        ]);
        let mut branch = original.clone();
        assert!(!branch.insert(EscErrorType::builtin("UnindexedBuiltin")));
        assert!(!branch.remove(&EscErrorType::UNKNOWN));
        branch.retain(|_| true);
        branch.join(original.clone());
        let (Types::Many(left), Types::Many(right)) = (&original, &branch) else {
            panic!("Expected shared unions")
        };
        assert!(Arc::ptr_eq(left, right));
        branch.insert(EscErrorType::UNKNOWN);
        assert!(!original.contains(&EscErrorType::UNKNOWN));
        branch.remove(&EscErrorType::builtin("UnindexedBuiltin"));
        assert!(original.contains(&EscErrorType::builtin("UnindexedBuiltin")));
        assert!(matches!(branch, Types::Builtins(_)));
        let mut drained = original.clone();
        assert_eq!(drained.drain().collect::<Types>(), original);
        assert!(drained.is_empty());
        assert_eq!(original.len(), 2);
    }
}
