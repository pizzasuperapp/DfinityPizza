//! Code for the computation of rewards distributions.
//!
//! This module makes use of floating-point computations. This is a reasonable
//! choice for computing rewards because:
//!
//! * Floating-point computations are deterministic and fully specified in wasm.
//!   In particular, rounding behavior is fully specified: https://www.w3.org/TR/wasm-core-1/#floating-point-operations%E2%91%A0
//!
//! * Floating-point operations are allowed in canister code.
//!
//! * The computation here happens pre-minting, and therefore there is no
//!   constraint that mandate fixed-precision.
//!
//! * Floating point makes code easier since the reward pool is specified as a
//!   fraction of the total Token supply.

use ic_nervous_system_common::{i2r, percent};
use num::{bigint::BigInt, rational::Ratio};
use std::ops::{Add, Div, Mul, Sub};

// ---- NON-BOILERPLATE CODE STARTS HERE ----------------------------------
// Because this module has very little "real" code and a lot of boilerplate
// and comments, all the interesting code is grouped here at the top.

// A timestamp in IC time -- that is, relative to the initialization of the
// governance canister.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct IcTimestamp {
    pub days_since_ic_genesis: Ratio<BigInt>,
}
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Duration {
    pub days: Ratio<BigInt>,
}
/// A dimensionless quantity divided by a duration.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct InverseDuration {
    pub per_day: Ratio<BigInt>,
}

// We can use actual operations to define the constants once https://github.com/rust-lang/rust/issues/67792
// becomes stable.
fn genesis() -> IcTimestamp {
    IcTimestamp {
        days_since_ic_genesis: i2r(0),
    }
}

fn average_days_per_year() -> Ratio<BigInt> {
    i2r(365) + percent(25)
}

fn one_day() -> Duration {
    Duration { days: i2r(1) }
}

/// The voting reward relative rate, at genesis.
///
/// "relative rate" means that it's a ratio where:
/// * the numerator is dimensionless (it's a fraction, to be multiplied by the
///   Token supply)
/// * the denominator is a duration
///
/// Note that this is not a "growth rate", at least not directly, because reward
/// distribution does not directly increase the Token supply. (It does indirectly,
/// when neuron owners spawn neurons). Therefore there is no automatic
/// compounding.
fn initial_voting_reward_relative_rate() -> InverseDuration {
    InverseDuration {
        per_day: percent(10) / average_days_per_year(), // 10% per year
    }
}

/// The voting reward relative rate, at the end of times.
///
/// See comment above for what "relative rate" precisely means.
fn final_voting_reward_relative_rate() -> InverseDuration {
    InverseDuration {
        per_day: percent(5) / average_days_per_year(), // 5% per year
    }
}

/// The date at which the reward rate reaches, and thereafter remains at, its
/// final value.
fn reward_flattening_date() -> IcTimestamp {
    IcTimestamp {
        days_since_ic_genesis: i2r(8) * average_days_per_year(),
    }
}

/// Computes the reward to distribute, as a fraction of the Token supply, for one
/// day.
pub fn rewards_pool_to_distribute_in_supply_fraction_for_one_day(
    days_since_ic_genesis: u64,
) -> Ratio<BigInt> {
    // Despite the rate being arguable a continuous function of time, we don't
    // integrate the rate here. Instead we multiply the rate at the beginning of
    // that day with the considered duration.
    let t = IcTimestamp {
        days_since_ic_genesis: i2r(days_since_ic_genesis),
    };

    let variable_rate = if t > reward_flattening_date() {
        InverseDuration { per_day: i2r(0) }
    } else {
        let duration_to_bottom = t - reward_flattening_date();
        let closeness_to_bottom = duration_to_bottom / (genesis() - reward_flattening_date());
        let delta_rate =
            initial_voting_reward_relative_rate() - final_voting_reward_relative_rate();
        closeness_to_bottom.pow(2) * delta_rate
    };

    let rate = final_voting_reward_relative_rate() + variable_rate;

    rate * one_day()
}

// ---- REAL-CODE ENDS HERE ---------------------------------------------

// Explication for the implementation of
// `rewards_pool_to_distribute_in_supply_fraction`
//
// The relevant extract from the spec is:
//
// -------------------------------------------------------------------------
// We derive the nominal maximum quantity of Tokens that can be
// minted and distributed as rewards from the current Token supply and
// the days since Genesis. To begin with, this is equal to 10% of the
// Token supply divided by the number of days in the year (365 normally,
// 366 in a leap year). Over 8 years, this falls to 5%. Note that since
// the supply of Tokens might grow (or even in theory fall) during this time,
// voting rewards may not halve in practice.
//
// * We want the rate at genesis to be 10% per year
// * We want the rate at genesis + 8 years to be 5% per year, and to be flat
//   thereafter
// * We want the rate to be a quadratic function of time
// * We want the rate to be differentiable wrt time at the point where it
//   becomes flat
// -------------------------------------------------------------------------
//
// Calling R0 the initial rate at genesis time G, Rf the final rate, and T the
// time at which the rate becomes flat, the unique solution is:
//
// R(t) = Rf + (R0-Rf) [ (t-T) / (G-T) ]^2
//
// Note that:
// R(G) = Rf + (R0-Rf) [ (G- T) / (G-T) ] ^ 2 = Rf + (R0-Rf) = R0
// R(T) = Rf + (R0-Rf) [ (T- T) / (G-T) ] ^ 2 = Rf
// R'(t) = 2 (R0-Rf)  (t-T) / ( G-T )^2
// R'(T) = 0
//
// ---- BOILERPLATE CODE STARTS HERE ----------------------------------
// There's no way in Rust to derive Add, Sub, etc. So we must have a ton of
// boilerplate for arithmetic. The rest of this module is boring stuff.

impl Sub for IcTimestamp {
    type Output = Duration;
    fn sub(self, other: IcTimestamp) -> Self::Output {
        Duration {
            days: self.days_since_ic_genesis - other.days_since_ic_genesis,
        }
    }
}
impl Mul<Duration> for InverseDuration {
    type Output = Ratio<BigInt>;
    fn mul(self, other: Duration) -> Self::Output {
        self.per_day * other.days
    }
}
impl Mul<InverseDuration> for Duration {
    type Output = Ratio<BigInt>;
    fn mul(self, other: InverseDuration) -> Self::Output {
        self.days * other.per_day
    }
}
impl Sub for InverseDuration {
    type Output = InverseDuration;
    fn sub(self, other: InverseDuration) -> Self::Output {
        InverseDuration {
            per_day: self.per_day - other.per_day,
        }
    }
}
impl Add for InverseDuration {
    type Output = InverseDuration;
    fn add(self, other: InverseDuration) -> Self::Output {
        InverseDuration {
            per_day: self.per_day + other.per_day,
        }
    }
}
impl Mul<InverseDuration> for Ratio<BigInt> {
    type Output = InverseDuration;
    fn mul(self, other: InverseDuration) -> Self::Output {
        InverseDuration {
            per_day: self * other.per_day,
        }
    }
}

impl Div<Duration> for Duration {
    type Output = Ratio<BigInt>;
    fn div(self, other: Duration) -> Self::Output {
        self.days / other.days
    }
}

// Surprisingly, Clippy complains that the `use` statement and the
// assert_approx_eq! macro are unused. This is very strange, so
// just tell clippy to keep quiet.
#[allow(unused_imports, unused_macros)]
mod test {
    use super::*;
    use ic_nervous_system_common::try_r2u64;

    #[test]
    fn days_fully_after_flattening_produce_linar_reward() {
        let expected = percent(5) / average_days_per_year();
        assert_eq!(
            rewards_pool_to_distribute_in_supply_fraction_for_one_day(8 * 366),
            expected,
        );
        assert_eq!(
            rewards_pool_to_distribute_in_supply_fraction_for_one_day(8 * 366 + 5),
            expected,
        );
        assert_eq!(
            rewards_pool_to_distribute_in_supply_fraction_for_one_day(123456),
            expected,
        );
    }

    #[test]
    fn reward_for_first_day() {
        assert_eq!(
            rewards_pool_to_distribute_in_supply_fraction_for_one_day(0),
            percent(10) / average_days_per_year(),
        );
    }

    #[test]
    fn reward_for_entire_pre_flattening_interval_can_be_lower_and_upper_bounded() {
        let lower_bound =
            (reward_flattening_date() - genesis()) * final_voting_reward_relative_rate();
        let upper_bound =
            (reward_flattening_date() - genesis()) * initial_voting_reward_relative_rate();
        let days_after_genesis =
            try_r2u64(&reward_flattening_date().days_since_ic_genesis).unwrap();
        let actual = (0..days_after_genesis)
            .map(rewards_pool_to_distribute_in_supply_fraction_for_one_day)
            .sum();
        assert!(lower_bound < actual);
        assert!(actual < upper_bound);
    }

    #[test]
    fn reward_is_convex_and_decreasing() {
        // Here we verify the convex inequality for all 3 consecutive days during the
        // parabolic rate period.
        let days_after_genesis =
            try_r2u64(&reward_flattening_date().days_since_ic_genesis).unwrap() - 2;
        for day in 0..days_after_genesis {
            let a = rewards_pool_to_distribute_in_supply_fraction_for_one_day(day);
            let b = rewards_pool_to_distribute_in_supply_fraction_for_one_day(day + 1);
            let c = rewards_pool_to_distribute_in_supply_fraction_for_one_day(day + 2);
            // First "derivative" is negative.
            assert!(a > b);
            assert!(b > c);
            // Second "derivative" is positive.
            assert!(a + c > i2r(2) * b);
        }
    }
}
