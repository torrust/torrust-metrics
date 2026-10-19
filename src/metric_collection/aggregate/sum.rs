use crate::counter::Counter;
use crate::gauge::Gauge;
use crate::label::LabelSet;
use crate::metric::MetricName;
use crate::metric::aggregate::sum::Sum as MetricSumTrait;
use crate::metric_collection::{MetricCollection, MetricKindCollection};

pub trait Sum {
    fn sum(&self, metric_name: &MetricName, label_set_criteria: &LabelSet) -> Option<f64>;
}

/// Exact integer sum of a counter's samples.
///
/// Unlike [`Sum`], which returns an `f64` for both counters and gauges, this
/// trait returns the exact `u64` total, so values above `2^53` are not rounded.
///
/// It returns `None` if there is no counter with the given name. A metric with
/// that name that is a gauge is not a counter, so it also returns `None`.
///
/// The addition is not checked: it panics on overflow in debug builds and
/// wraps in release builds, like [`Metric<Counter>::sum`](MetricSumTrait::sum).
///
/// # Example
///
/// ```
/// use torrust_clock::DurationSinceUnixEpoch;
/// use torrust_metrics::label::LabelSet;
/// use torrust_metrics::metric_collection::MetricCollection;
/// use torrust_metrics::metric_collection::aggregate::sum::SumCounter;
/// use torrust_metrics::metric_name;
///
/// let mut collection = MetricCollection::default();
/// let name = metric_name!("requests_total");
/// let time = DurationSinceUnixEpoch::from_secs(1);
///
/// collection.set_counter(&name, &LabelSet::empty(), (1 << 53) + 1, time).unwrap();
///
/// assert_eq!(collection.sum_counter(&name, &LabelSet::empty()), Some((1 << 53) + 1));
/// ```
pub trait SumCounter {
    fn sum_counter(&self, metric_name: &MetricName, label_set_criteria: &LabelSet) -> Option<u64>;
}

impl SumCounter for MetricCollection {
    fn sum_counter(&self, metric_name: &MetricName, label_set_criteria: &LabelSet) -> Option<u64> {
        self.counters
            .metrics
            .get(metric_name)
            .map(|metric| metric.sum(label_set_criteria))
    }
}

impl Sum for MetricCollection {
    fn sum(&self, metric_name: &MetricName, label_set_criteria: &LabelSet) -> Option<f64> {
        if let Some(value) = self.counters.sum(metric_name, label_set_criteria) {
            return Some(value);
        }

        if let Some(value) = self.gauges.sum(metric_name, label_set_criteria) {
            return Some(value);
        }

        None
    }
}

impl Sum for MetricKindCollection<Counter> {
    fn sum(&self, metric_name: &MetricName, label_set_criteria: &LabelSet) -> Option<f64> {
        #[allow(clippy::cast_precision_loss)]
        self.metrics
            .get(metric_name)
            .map(|metric| metric.sum(label_set_criteria) as f64)
    }
}

impl Sum for MetricKindCollection<Gauge> {
    fn sum(&self, metric_name: &MetricName, label_set_criteria: &LabelSet) -> Option<f64> {
        self.metrics.get(metric_name).map(|metric| metric.sum(label_set_criteria))
    }
}

#[cfg(test)]
mod tests {

    mod it_should_allow_summing_all_metric_samples_containing_some_given_labels {

        use torrust_clock::DurationSinceUnixEpoch;

        use crate::label::LabelValue;
        use crate::label_name;
        use crate::metric_collection::aggregate::sum::Sum;

        #[test]
        fn type_counter_with_two_samples() {
            use crate::label::LabelSet;
            use crate::metric_collection::MetricCollection;
            use crate::metric_name;

            let metric_name = metric_name!("test_counter");

            let mut collection = MetricCollection::default();

            collection
                .increment_counter(
                    &metric_name!("test_counter"),
                    &(label_name!("label_1"), LabelValue::new("value_1")).into(),
                    DurationSinceUnixEpoch::from_secs(1),
                )
                .unwrap();

            collection
                .increment_counter(
                    &metric_name!("test_counter"),
                    &(label_name!("label_2"), LabelValue::new("value_2")).into(),
                    DurationSinceUnixEpoch::from_secs(1),
                )
                .unwrap();

            assert_eq!(collection.sum(&metric_name, &LabelSet::empty()), Some(2.0));
            assert_eq!(
                collection.sum(&metric_name, &(label_name!("label_1"), LabelValue::new("value_1")).into()),
                Some(1.0)
            );
        }

        #[test]
        fn type_gauge_with_two_samples() {
            use crate::label::LabelSet;
            use crate::metric_collection::MetricCollection;
            use crate::metric_name;

            let metric_name = metric_name!("test_gauge");

            let mut collection = MetricCollection::default();

            collection
                .increment_gauge(
                    &metric_name!("test_gauge"),
                    &(label_name!("label_1"), LabelValue::new("value_1")).into(),
                    DurationSinceUnixEpoch::from_secs(1),
                )
                .unwrap();

            collection
                .increment_gauge(
                    &metric_name!("test_gauge"),
                    &(label_name!("label_2"), LabelValue::new("value_2")).into(),
                    DurationSinceUnixEpoch::from_secs(1),
                )
                .unwrap();

            assert_eq!(collection.sum(&metric_name, &LabelSet::empty()), Some(2.0));
            assert_eq!(
                collection.sum(&metric_name, &(label_name!("label_1"), LabelValue::new("value_1")).into()),
                Some(1.0)
            );
        }

        #[test]
        fn nonexistent_counter_metric_returns_none() {
            use crate::label::LabelSet;
            use crate::metric_collection::MetricCollection;
            use crate::metric_name;

            let collection = MetricCollection::default();

            assert_eq!(collection.sum(&metric_name!("does_not_exist"), &LabelSet::empty()), None);
        }

        #[test]
        fn nonexistent_gauge_metric_returns_none() {
            use crate::label::LabelSet;
            use crate::metric_collection::MetricCollection;
            use crate::{label_name, metric_name};

            let mut collection = MetricCollection::default();

            // Add a counter (not a gauge) so gauges map remains empty for this name
            collection
                .increment_counter(
                    &metric_name!("some_counter"),
                    &(label_name!("x"), LabelValue::new("y")).into(),
                    DurationSinceUnixEpoch::from_secs(1),
                )
                .unwrap();

            assert_eq!(collection.sum(&metric_name!("missing_gauge"), &LabelSet::empty()), None);
        }
    }

    mod it_should_allow_summing_counter_samples_exactly {

        use torrust_clock::DurationSinceUnixEpoch;

        use crate::label::{LabelSet, LabelValue};
        use crate::metric_collection::MetricCollection;
        use crate::metric_collection::aggregate::sum::{Sum, SumCounter};
        use crate::{label_name, metric_name};

        fn time() -> DurationSinceUnixEpoch {
            DurationSinceUnixEpoch::from_secs(1)
        }

        #[test]
        fn with_several_samples_and_a_label_filter() {
            let name = metric_name!("test_counter");
            let label_1: LabelSet = (label_name!("label_1"), LabelValue::new("value_1")).into();
            let label_2: LabelSet = (label_name!("label_2"), LabelValue::new("value_2")).into();

            let mut collection = MetricCollection::default();
            collection.set_counter(&name, &label_1, 3, time()).unwrap();
            collection.set_counter(&name, &label_2, 4, time()).unwrap();

            assert_eq!(collection.sum_counter(&name, &label_1), Some(3));
            assert_eq!(collection.sum_counter(&name, &label_2), Some(4));
        }

        #[test]
        fn summing_every_sample_when_the_label_set_is_empty() {
            let name = metric_name!("test_counter");

            let mut collection = MetricCollection::default();
            collection
                .set_counter(&name, &(label_name!("label_1"), LabelValue::new("value_1")).into(), 3, time())
                .unwrap();
            collection
                .set_counter(&name, &(label_name!("label_2"), LabelValue::new("value_2")).into(), 4, time())
                .unwrap();

            assert_eq!(collection.sum_counter(&name, &LabelSet::empty()), Some(7));
        }

        #[test]
        fn returning_none_when_the_metric_does_not_exist() {
            let collection = MetricCollection::default();

            assert_eq!(
                collection.sum_counter(&metric_name!("does_not_exist"), &LabelSet::empty()),
                None
            );
        }

        #[test]
        fn returning_none_when_the_metric_is_a_gauge() {
            let name = metric_name!("test_gauge");

            let mut collection = MetricCollection::default();
            collection.set_gauge(&name, &LabelSet::empty(), 1.0, time()).unwrap();

            assert_eq!(collection.sum_counter(&name, &LabelSet::empty()), None);
        }

        #[test]
        fn returning_values_above_two_to_the_power_of_fifty_three_exactly() {
            let name = metric_name!("test_counter");
            let value = (1_u64 << 53) + 1;

            let mut collection = MetricCollection::default();
            collection.set_counter(&name, &LabelSet::empty(), value, time()).unwrap();

            assert_eq!(collection.sum_counter(&name, &LabelSet::empty()), Some(value));

            // The `f64` based `Sum` rounds the same value down to `2^53`.
            assert_eq!(collection.sum(&name, &LabelSet::empty()), Some(9_007_199_254_740_992.0));
        }
    }
}
