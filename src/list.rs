/// A fixed-capacity double-ended queue of at most `N` values of type
/// `T`, fully usable from `const` contexts. Backed by a ring buffer, so
/// values can be pushed and popped from either the front or the back
/// without shifting the rest of the elements.
#[derive(Clone, Copy)]
pub struct ConstList<T, const N: usize>
where
    T: Copy,
{
    values: [Option<T>; N],
    /// Physical index of the front-most element (meaningless when `len == 0`).
    start: usize,
    len: usize,
}

impl<T, const N: usize> ConstList<T, { N }>
where
    T: Copy,
{
    /// Creates an empty list with capacity `N`.
    pub const fn new() -> Self {
        Self {
            values: [None; N],
            start: 0,
            len: 0,
        }
    }

    /// Returns the number of values currently stored in the list, i.e.
    /// its current length, not its fixed capacity `N`.
    #[allow(unused)]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Maps a logical index (`0` = front) to its physical slot in
    /// `values`. Only meaningful when `index < self.len`.
    const fn physical_index(&self, index: usize) -> usize {
        (self.start + index) % N
    }

    /// Appends `value` to the back of the list.
    ///
    /// # Panics
    ///
    /// Panics if the list is already at capacity `N`.
    pub const fn push_back(&mut self, value: T) {
        if self.len >= N {
            panic!("ConstList exceeded size");
        }

        let phys = self.physical_index(self.len);
        self.values[phys] = Some(value);
        self.len += 1;
    }

    /// Prepends `value` to the front of the list.
    ///
    /// # Panics
    ///
    /// Panics if the list is already at capacity `N`.
    #[allow(unused)]
    pub const fn push_front(&mut self, value: T) {
        if self.len >= N {
            panic!("ConstList exceeded size");
        }

        self.start = (self.start + N - 1) % N;
        self.values[self.start] = Some(value);
        self.len += 1;
    }

    /// Removes and returns the value at the back of the list, or
    /// `None` if the list is empty.
    #[allow(unused)]
    pub const fn pop_back(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            let phys = self.physical_index(self.len);
            self.values[phys]
        }
    }

    /// Removes and returns the value at the front of the list, or
    /// `None` if the list is empty.
    pub const fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            let value = self.values[self.start];
            self.start = (self.start + 1) % N;
            self.len -= 1;
            value
        }
    }

    /// Appends every value from `other` to the back of `self`, front to
    /// back.
    ///
    /// # Panics
    ///
    /// Panics if `other` has more values than there is remaining
    /// capacity for, i.e. if `self.len() + other.len() > N`.
    pub const fn extend(&mut self, other: Self) {
        if self.len + other.len > N {
            panic!("ConstList exceeded size");
        }

        let mut i = 0;
        while i < other.len {
            let phys = self.physical_index(self.len + i);
            self.values[phys] = other.values[other.physical_index(i)];
            i += 1;
        }

        self.len += other.len;
    }
}

#[cfg(test)]
mod const_list_tests {
    use super::*;

    /// Reads the logical element at `index` by inspecting the backing
    /// store directly. Stands in for the removed public `get` accessor
    /// so the tests can still assert on logical contents.
    fn logical<T: Copy, const N: usize>(list: &ConstList<T, N>, index: usize) -> Option<T> {
        if index < list.len {
            list.values[list.physical_index(index)]
        } else {
            None
        }
    }

    #[test]
    fn new_initializes_empty_list() {
        let list: ConstList<i32, 4> = ConstList::new();
        assert_eq!(list.len(), 0);
        assert_eq!(list.values, [None, None, None, None]);
    }

    #[test]
    fn len_is_zero_for_a_new_list() {
        let list: ConstList<i32, 4> = ConstList::new();
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn len_reflects_current_element_count_not_capacity() {
        let mut list: ConstList<i32, 4> = ConstList::new();
        list.push_back(1);
        assert_eq!(list.len(), 1);
        list.push_back(2);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn len_decreases_after_pop_back() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        list.pop_back();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn len_decreases_after_pop_front() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        list.pop_front();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn len_reflects_combined_count_after_extend() {
        let mut a: ConstList<i32, 5> = ConstList::new();
        a.push_back(1);

        let mut b: ConstList<i32, 5> = ConstList::new();
        b.push_back(2);
        b.push_back(3);

        a.extend(b);
        assert_eq!(a.len(), 3);
    }

    #[test]
    fn push_back_appends_value() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(42);
        assert_eq!(list.len(), 1);
        assert_eq!(logical(&list, 0), Some(42));
    }

    #[test]
    fn push_back_fills_list_in_order() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        assert_eq!(logical(&list, 0), Some(1));
        assert_eq!(logical(&list, 1), Some(2));
        assert_eq!(logical(&list, 2), Some(3));
        assert_eq!(list.len(), 3);
    }

    #[test]
    #[should_panic(expected = "ConstList exceeded size")]
    fn push_back_panics_once_capacity_is_reached() {
        let mut list: ConstList<i32, 2> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
    }

    #[test]
    fn push_front_prepends_value() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(2);
        list.push_front(1);
        assert_eq!(list.len(), 2);
        assert_eq!(logical(&list, 0), Some(1));
        assert_eq!(logical(&list, 1), Some(2));
    }

    #[test]
    fn push_front_builds_list_in_reverse_order() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_front(1);
        list.push_front(2);
        list.push_front(3);
        assert_eq!(logical(&list, 0), Some(3));
        assert_eq!(logical(&list, 1), Some(2));
        assert_eq!(logical(&list, 2), Some(1));
    }

    #[test]
    #[should_panic(expected = "ConstList exceeded size")]
    fn push_front_panics_once_capacity_is_reached() {
        let mut list: ConstList<i32, 2> = ConstList::new();
        list.push_front(1);
        list.push_front(2);
        list.push_front(3);
    }

    #[test]
    fn pop_back_removes_and_returns_values_in_lifo_order() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list.pop_back(), Some(2));
        assert_eq!(list.pop_back(), Some(1));
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn pop_back_returns_none_when_list_is_empty() {
        let mut list: ConstList<i32, 2> = ConstList::new();
        assert_eq!(list.pop_back(), None);
    }

    #[test]
    fn pop_front_removes_and_returns_values_in_fifo_order() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list.pop_front(), Some(1));
        assert_eq!(list.pop_front(), Some(2));
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn pop_front_returns_none_when_list_is_empty() {
        let mut list: ConstList<i32, 2> = ConstList::new();
        assert_eq!(list.pop_front(), None);
    }

    #[test]
    fn push_back_after_pop_front_wraps_around_the_ring_buffer() {
        let mut list: ConstList<i32, 2> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list.pop_front(), Some(1));
        // the freed physical slot is at the start of the array, so this
        // push has to wrap around to reuse it.
        list.push_back(3);
        assert_eq!(list.len(), 2);
        assert_eq!(logical(&list, 0), Some(2));
        assert_eq!(logical(&list, 1), Some(3));
    }

    #[test]
    fn push_front_after_pop_back_wraps_around_the_ring_buffer() {
        let mut list: ConstList<i32, 2> = ConstList::new();
        list.push_front(1);
        list.push_front(2);
        assert_eq!(list.pop_back(), Some(1));
        list.push_front(3);
        assert_eq!(list.len(), 2);
        assert_eq!(logical(&list, 0), Some(3));
        assert_eq!(logical(&list, 1), Some(2));
    }

    #[test]
    fn mixed_front_and_back_pushes_preserve_logical_order() {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(2);
        list.push_front(1);
        list.push_back(3);
        assert_eq!(logical(&list, 0), Some(1));
        assert_eq!(logical(&list, 1), Some(2));
        assert_eq!(logical(&list, 2), Some(3));
        assert_eq!(list.pop_front(), Some(1));
        assert_eq!(list.pop_front(), Some(2));
        assert_eq!(list.pop_front(), Some(3));
    }

    #[test]
    fn extend_appends_all_values_from_other_list_in_order() {
        let mut a: ConstList<i32, 5> = ConstList::new();
        a.push_back(1);
        a.push_back(2);

        let mut b: ConstList<i32, 5> = ConstList::new();
        b.push_back(3);
        b.push_back(4);

        a.extend(b);
        assert_eq!(a.len(), 4);
        assert_eq!(logical(&a, 0), Some(1));
        assert_eq!(logical(&a, 1), Some(2));
        assert_eq!(logical(&a, 2), Some(3));
        assert_eq!(logical(&a, 3), Some(4));
    }

    #[test]
    fn extend_succeeds_when_result_exactly_fills_capacity() {
        let mut a: ConstList<i32, 5> = ConstList::new();
        a.push_back(1);
        a.push_back(2);

        let mut b: ConstList<i32, 5> = ConstList::new();
        b.push_back(3);
        b.push_back(4);
        b.push_back(5);

        a.extend(b);
        assert_eq!(a.len(), 5);
        for (i, expected) in [1, 2, 3, 4, 5].into_iter().enumerate() {
            assert_eq!(logical(&a, i), Some(expected));
        }
    }

    #[test]
    #[should_panic(expected = "ConstList exceeded size")]
    fn extend_panics_when_result_would_exceed_capacity() {
        let mut a: ConstList<i32, 3> = ConstList::new();
        a.push_back(1);
        a.push_back(2);

        let mut b: ConstList<i32, 3> = ConstList::new();
        b.push_back(3);
        b.push_back(4);

        a.extend(b);
    }

    #[test]
    fn extend_appends_correctly_when_either_side_has_wrapped_around() {
        // `a` wraps: two front-pushes leave `start` at a non-zero index,
        // then a back-pop shrinks `len` without resetting `start`.
        let mut a: ConstList<i32, 3> = ConstList::new();
        a.push_front(2);
        a.push_front(1);
        a.pop_back();

        // `b` also wraps via a front push.
        let mut b: ConstList<i32, 3> = ConstList::new();
        b.push_back(10);
        b.push_front(9);

        a.extend(b);
        assert_eq!(a.len(), 3);
        assert_eq!(logical(&a, 0), Some(1));
        assert_eq!(logical(&a, 1), Some(9));
        assert_eq!(logical(&a, 2), Some(10));
    }

    // `ConstList` is meant to be fully usable at compile time, so exercise
    // its methods inside an actual `const` context, not just at runtime.
    const _: () = {
        let mut list: ConstList<i32, 3> = ConstList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        assert!(list.len() == 3);
    };

    const _: () = {
        let mut list: ConstList<i32, 2> = ConstList::new();
        list.push_back(10);
        list.push_back(20);
        assert!(matches!(list.pop_back(), Some(20)));
        assert!(matches!(list.pop_back(), Some(10)));
        assert!(matches!(list.pop_back(), None));
    };

    const _: () = {
        let mut list: ConstList<i32, 2> = ConstList::new();
        list.push_front(10);
        list.push_front(20);
        assert!(matches!(list.pop_front(), Some(20)));
        assert!(matches!(list.pop_front(), Some(10)));
        assert!(matches!(list.pop_front(), None));
    };

    const _: () = {
        let mut a: ConstList<i32, 4> = ConstList::new();
        a.push_back(1);
        a.push_back(2);

        let mut b: ConstList<i32, 4> = ConstList::new();
        b.push_back(3);

        a.extend(b);
        assert!(a.len() == 3);
    };

    const _: () = {
        let mut list: ConstList<i32, 3> = ConstList::new();
        assert!(list.len() == 0);
        list.push_back(1);
        list.push_back(2);
        assert!(list.len() == 2);
        list.pop_back();
        assert!(list.len() == 1);
    };
}
