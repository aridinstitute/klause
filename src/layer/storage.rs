use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use core::{cell::OnceCell, cmp::Ordering, num::NonZeroU32, ops::Range};

use crate::{Span, SpanId, SpanRef, Value};

/// A flat column of variable-length rows. Until a row differs in width from
/// the first one, its offset is implicit. In particular, one range per span
/// and zero attributes per span require no offset allocation at all.
pub(super) struct Rows<T> {
    data: Vec<T>,
    offsets: Vec<u32>,
    width: usize,
    len: usize,
}

impl<T> Default for Rows<T> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            offsets: Vec::new(),
            width: 0,
            len: 0,
        }
    }
}
impl<T> Rows<T> {
    fn bounds(&self, slot: usize) -> Range<usize> {
        if self.offsets.is_empty() {
            slot * self.width..(slot + 1) * self.width
        } else {
            self.offsets[slot] as usize..self.offsets[slot + 1] as usize
        }
    }

    pub(super) fn get(&self, slot: usize) -> &[T] {
        &self.data[self.bounds(slot)]
    }

    pub(super) fn set(&mut self, slot: usize, items: impl ExactSizeIterator<Item = T>) {
        self.write(slot, items.len(), items);
    }

    fn write(&mut self, slot: usize, width: usize, items: impl Iterator<Item = T>) {
        if self.len == 0 {
            self.width = width;
        }
        if self.offsets.is_empty() && width != self.width {
            self.offsets = (0..=self.len).map(|i| offset(i * self.width)).collect();
        }
        if slot == self.len {
            let end = offset(self.data.len() + width);
            self.data.reserve(width);
            self.data.extend(items);
            if !self.offsets.is_empty() {
                self.offsets.push(end);
            }
            self.len += 1;
        } else {
            let bounds = self.bounds(slot);
            let old_width = bounds.len();
            if width == old_width {
                for (stored, value) in self.data[bounds].iter_mut().zip(items) {
                    *stored = value;
                }
            } else {
                offset(self.data.len() - old_width + width);
                self.data.splice(bounds, items);
                for end in &mut self.offsets[slot + 1..] {
                    *end = offset(*end as usize - old_width + width);
                }
            }
        }
    }
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).expect("layer column exceeds u32::MAX entries")
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum IdRef<'a> {
    Int(u64),
    Str(&'a str),
}

impl<'a> From<&'a SpanId> for IdRef<'a> {
    fn from(id: &'a SpanId) -> Self {
        match id {
            SpanId::Int(id) => Self::Int(*id),
            SpanId::Str(id) => Self::Str(id),
        }
    }
}

impl From<IdRef<'_>> for SpanId {
    fn from(id: IdRef<'_>) -> Self {
        match id {
            IdRef::Int(id) => Self::Int(id),
            IdRef::Str(id) => Self::Str(id.into()),
        }
    }
}

#[derive(Default)]
pub(super) struct Ids {
    integers: Vec<u64>,
    // Normally a sorted integer ID's index is its slot. Only out-of-order
    // inserts, mixed IDs, and holes can require explicit forward/reverse maps.
    indirect: Option<IndirectIds>,
    strings: BTreeMap<Arc<str>, usize>,
    slot_strings: BTreeMap<usize, Arc<str>>,
}

struct IndirectIds {
    slots: Vec<u32>,
    integers_by_slot: Vec<u64>,
}

impl Ids {
    fn make_indirect(&mut self) -> &mut IndirectIds {
        self.indirect.get_or_insert_with(|| IndirectIds {
            slots: (0..self.integers.len()).map(offset).collect(),
            integers_by_slot: self.integers.clone(),
        })
    }

    pub(super) fn slot(&self, id: IdRef<'_>) -> Option<usize> {
        match id {
            IdRef::Int(id) => self.integers.binary_search(&id).ok().map(|index| {
                self.indirect
                    .as_ref()
                    .map_or(index, |map| map.slots[index] as usize)
            }),
            IdRef::Str(id) => self.strings.get(id).copied(),
        }
    }

    pub(super) fn id(&self, slot: usize) -> IdRef<'_> {
        if let Some(id) = self.slot_strings.get(&slot) {
            IdRef::Str(id)
        } else {
            IdRef::Int(match &self.indirect {
                None => self.integers[slot],
                Some(map) => map.integers_by_slot[slot],
            })
        }
    }

    pub(super) fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.integers.len())
            .map(|index| {
                self.indirect
                    .as_ref()
                    .map_or(index, |map| map.slots[index] as usize)
            })
            .chain(self.strings.values().copied())
    }

    pub(super) fn len(&self) -> usize {
        self.integers.len() + self.strings.len()
    }

    pub(super) fn insert(&mut self, id: SpanId, slot: usize) {
        match id {
            SpanId::Int(id) => {
                let index = if self.integers.last().is_none_or(|&last| last < id) {
                    self.integers.len()
                } else {
                    self.integers.binary_search(&id).unwrap_err()
                };
                if index != self.integers.len() || slot != index || self.indirect.is_some() {
                    let map = self.make_indirect();
                    map.slots.insert(index, offset(slot));
                    if slot >= map.integers_by_slot.len() {
                        map.integers_by_slot.resize(slot + 1, 0);
                    }
                    map.integers_by_slot[slot] = id;
                }
                self.integers.insert(index, id);
            }
            SpanId::Str(id) => {
                let id = Arc::<str>::from(id);
                self.strings.insert(id.clone(), slot);
                self.slot_strings.insert(slot, id);
            }
        }
    }

    pub(super) fn remove(&mut self, slot: usize, next_id: &mut Option<u64>) {
        if let Some(id) = self.slot_strings.remove(&slot) {
            self.strings.remove(&id);
        } else {
            let IdRef::Int(id) = self.id(slot) else {
                unreachable!()
            };
            let index = self.integers.binary_search(&id).unwrap();
            if index + 1 < self.integers.len() || self.indirect.is_some() {
                self.make_indirect().slots.remove(index);
            }
            self.integers.remove(index);
            if id.checked_add(1) == *next_id {
                *next_id = self
                    .integers
                    .last()
                    .map_or(Some(1), |max| max.checked_add(1));
            }
        }
    }

    pub(super) fn compare(&self, a: usize, b: usize) -> Ordering {
        self.id(a).cmp(&self.id(b))
    }
}

#[derive(Default)]
pub(super) struct Targets {
    // Parallel annotation layers commonly target the same ID in one other
    // layer. Store that relationship once, and record deviations by slot.
    same_id_layer: Option<String>,
    overrides: BTreeMap<u32, Option<NonZeroU32>>,
    references: Vec<SpanRef>,
    reference_codes: BTreeMap<String, BTreeMap<SpanId, NonZeroU32>>,
    between: BTreeMap<u32, (SpanRef, SpanRef)>,
}

impl Targets {
    pub(super) fn implicit_layer(&self) -> Option<&str> {
        self.same_id_layer.as_deref()
    }

    pub(super) fn is_implicit(&self, slot: usize) -> bool {
        self.same_id_layer.is_some() && !self.overrides.contains_key(&(slot as u32))
    }

    pub(super) fn explicit(&self, slot: usize) -> Option<&SpanRef> {
        let code = self.overrides.get(&(slot as u32)).copied().flatten()?;
        Some(&self.references[code.get() as usize - 1])
    }

    pub(super) fn between(&self, slot: usize) -> Option<&(SpanRef, SpanRef)> {
        self.between.get(&(slot as u32))
    }

    pub(super) fn set_between(
        &mut self,
        slot: usize,
        id: IdRef<'_>,
        first: bool,
        start: SpanRef,
        end: SpanRef,
    ) {
        self.set(slot, id, first, None);
        self.between.insert(slot as u32, (start, end));
    }
    pub(super) fn set(&mut self, slot: usize, id: IdRef<'_>, first: bool, target: Option<SpanRef>) {
        self.between.remove(&(slot as u32));
        if first {
            self.same_id_layer = target
                .as_ref()
                .filter(|r| IdRef::from(r.id()) == id)
                .map(|r| r.layer().into());
        }
        if match &target {
            None => self.same_id_layer.is_none(),
            Some(r) => self.implicit_layer() == Some(r.layer()) && IdRef::from(r.id()) == id,
        } {
            self.remove(slot);
        } else {
            let code = target.map(|r| self.intern(r));
            self.overrides.insert(slot as u32, code);
        }
    }

    fn intern(&mut self, reference: SpanRef) -> NonZeroU32 {
        if let Some(&code) = self
            .reference_codes
            .get(reference.layer())
            .and_then(|ids| ids.get(reference.id()))
        {
            return code;
        }
        let code = NonZeroU32::new(offset(self.references.len() + 1)).unwrap();
        self.reference_codes
            .entry(reference.layer().into())
            .or_default()
            .insert(reference.id().clone(), code);
        self.references.push(reference);
        code
    }
    pub(super) fn remove(&mut self, slot: usize) {
        self.between.remove(&(slot as u32));
        self.overrides.remove(&(slot as u32));
    }
}

#[derive(Default)]
pub(super) struct AttributeColumns {
    rows: Rows<(u32, u32)>,
    keys: Vec<String>,
    key_codes: BTreeMap<String, u32>,
    values: Vec<Value>,
    string_codes: BTreeMap<String, u32>,
    scalar_codes: BTreeMap<Scalar, u32>,
}

// Use exact representations for interning, distinct from Value equality.
// In particular, materialization must preserve signed zero and NaN payloads.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Scalar {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
}

impl Scalar {
    fn of(value: &Value) -> Self {
        match value {
            Value::Null => Self::Null,
            Value::Bool(value) => Self::Bool(*value),
            Value::Int(value) => Self::Int(*value),
            Value::Float(value) => Self::Float(value.to_bits()),
            Value::Str(_) => unreachable!(),
        }
    }
}

impl AttributeColumns {
    pub(super) fn set(&mut self, slot: usize, attrs: Vec<(String, Vec<Value>)>) {
        // Most rows have just a couple of pairs; reuse the owned input values
        // and write the codes directly to the flat column.
        let len = attrs.iter().map(|(_, values)| values.len()).sum();
        let pairs = attrs.into_iter().flat_map(|(key, values)| {
            let next = offset(self.keys.len());
            let key = *self.key_codes.entry(key).or_insert_with_key(|key| {
                self.keys.push(key.clone());
                next
            });
            values.into_iter().map(move |value| (key, value))
        });
        let values = &mut self.values;
        let strings = &mut self.string_codes;
        let scalars = &mut self.scalar_codes;
        let codes = pairs.map(|(key, value)| {
            let next = offset(values.len());
            let code = match &value {
                Value::Str(string) => match strings.get(string) {
                    Some(&code) => code,
                    None => {
                        strings.insert(string.clone(), next);
                        values.push(value);
                        next
                    }
                },
                _ => *scalars.entry(Scalar::of(&value)).or_insert_with(|| {
                    values.push(value);
                    next
                }),
            };
            (key, code)
        });
        // Chain over attribute groups while still supplying the known row width.
        self.rows.write(slot, len, codes);
    }

    pub(super) fn key_code(&self, key: &str) -> Option<u32> {
        self.key_codes.get(key).copied()
    }

    pub(super) fn codes(&self, slot: usize) -> &[(u32, u32)] {
        self.rows.get(slot)
    }

    pub(super) fn attrs(&self, slot: usize) -> impl Iterator<Item = (&str, &Value)> {
        self.codes(slot).iter().map(|&(key, value)| {
            (
                self.keys[key as usize].as_str(),
                &self.values[value as usize],
            )
        })
    }

    pub(super) fn values(&self, slot: usize, key: u32) -> impl Iterator<Item = &Value> {
        self.codes(slot)
            .iter()
            .filter(move |&&(stored, _)| key == stored)
            .map(|&(_, value)| &self.values[value as usize])
    }

    pub(super) fn equal_codes(&self, value: &Value) -> impl Iterator<Item = u32> {
        let mut codes = [None; 3];
        let find = |scalar| self.scalar_codes.get(&scalar).copied();
        match value {
            Value::Str(string) => codes[0] = self.string_codes.get(string).copied(),
            Value::Int(value) => {
                codes[0] = find(Scalar::Int(*value));
                let float = *value as f64;
                if float < -(i64::MIN as f64) && float as i64 == *value {
                    codes[1] = find(Scalar::Float(float.to_bits()));
                    if *value == 0 {
                        codes[2] = find(Scalar::Float((-0.0_f64).to_bits()));
                    }
                }
            }
            Value::Float(value) => {
                codes[0] = find(Scalar::Float(value.to_bits()));
                if *value == 0.0 {
                    codes[1] = find(Scalar::Float((-value).to_bits()));
                }
                if *value >= i64::MIN as f64 && *value < -(i64::MIN as f64) {
                    let integer = *value as i64;
                    if integer as f64 == *value {
                        codes[2] = find(Scalar::Int(integer));
                    }
                }
            }
            _ => codes[0] = find(Scalar::of(value)),
        }
        codes.into_iter().flatten()
    }
}

// Borrowed public reads need stable addresses, but normal queries don't need
// these Spans at all. A lazily allocated radix tree keeps pages only for slots
// read through the borrowed API, without a pointer per stored span or unsafe.
#[derive(Default)]
pub(super) struct SpanCache(OnceCell<CacheNode>);

enum CacheNode {
    Branch(Box<[OnceCell<CacheNode>; 16]>),
    Leaf(Box<[OnceCell<Span>; 16]>),
}

impl SpanCache {
    pub(super) fn get_or_init(&self, slot: usize, build: impl FnOnce() -> Span) -> &Span {
        let mut node = &self.0;
        for shift in (1..8).rev() {
            let CacheNode::Branch(children) = node.get_or_init(|| {
                CacheNode::Branch(Box::new(core::array::from_fn(|_| OnceCell::new())))
            }) else {
                unreachable!()
            };
            node = &children[(slot >> (shift * 4)) & 15];
        }
        let CacheNode::Leaf(spans) = node
            .get_or_init(|| CacheNode::Leaf(Box::new(core::array::from_fn(|_| OnceCell::new()))))
        else {
            unreachable!()
        };
        spans[slot & 15].get_or_init(build)
    }

    pub(super) fn remove(&mut self, slot: usize) {
        fn remove(node: &mut OnceCell<CacheNode>, slot: usize, shift: usize) {
            let empty = match node.get_mut() {
                Some(CacheNode::Branch(children)) => {
                    remove(&mut children[(slot >> shift) & 15], slot, shift - 4);
                    children.iter().all(|child| child.get().is_none())
                }
                Some(CacheNode::Leaf(spans)) => {
                    spans[slot & 15].take();
                    spans.iter().all(|span| span.get().is_none())
                }
                None => false,
            };
            if empty {
                node.take();
            }
        }
        remove(&mut self.0, slot, 28);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_rows_need_only_the_flat_data() {
        let mut rows = Rows::default();
        for slot in 0..8 {
            rows.set(slot, core::iter::once(slot as u32..slot as u32 + 1));
        }
        assert_eq!(rows.data.len(), 8);
        assert!(rows.offsets.is_empty());

        rows.set(0, core::iter::empty());
        rows.set(2, [4..5, 7..8].into_iter());
        assert!(rows.get(0).is_empty());
        assert_eq!(rows.get(2), &[4..5, 7..8]);
        assert_eq!(rows.get(7).first(), Some(&(7..8)));
    }
}
