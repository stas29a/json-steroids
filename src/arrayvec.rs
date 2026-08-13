use arrayvec::ArrayVec;

use crate::{writer::Writer, JsonDeserialize, JsonSerialize, JsonWriter};

impl<T: JsonSerialize, const CAP: usize> JsonSerialize for ArrayVec<T, CAP> {
    #[inline]
    fn json_serialize<W: Writer>(&self, writer: &mut JsonWriter<W>) {
        self.as_slice().json_serialize(writer)
    }
}

impl<'de, T: JsonDeserialize<'de>, const CAP: usize> JsonDeserialize<'de> for ArrayVec<T, CAP> {
    #[inline]
    fn json_deserialize(parser: &mut crate::JsonParser<'de>) -> crate::Result<Self> {
        parser.expect_array_start()?;
        let mut result = ArrayVec::new();

        while parser.has_next_array_element_or_first(result.is_empty())? {
            result.push(T::json_deserialize(parser)?);
        }

        parser.expect_array_end()?;
        Ok(result)
    }
}

#[cfg(test)]
mod test {
    use crate::{from_str, to_string};
    use arrayvec::ArrayVec;

    #[test]
    fn simple() {
        let series = ArrayVec::from_iter([4, 3, 2, 1]);

        let serialized = to_string(&series);
        assert_eq!(serialized, "[4,3,2,1]");

        let deserialized: ArrayVec<i8, 4> = from_str(&serialized).unwrap();
        assert_eq!(deserialized, series);
    }
}
