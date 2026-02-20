#![allow(dead_code)]

use crate::effect::{Effect, TickContext};

// ---------------------------------------------------------------------------
// AddEffect – element-wise sum of N child effects (must share channel count)
// ---------------------------------------------------------------------------

pub struct AddEffect {
    children: Vec<Box<dyn Effect>>,
    channel_count: usize,
}

impl AddEffect {
    /// Panics if the children list is empty or children have mismatched channel
    /// counts.
    pub fn new(children: Vec<Box<dyn Effect>>) -> Self {
        assert!(!children.is_empty(), "AddEffect requires at least one child");
        let channel_count = children[0].channel_count();
        for c in &children {
            assert_eq!(
                c.channel_count(),
                channel_count,
                "AddEffect: all children must have the same channel count"
            );
        }
        Self {
            children,
            channel_count,
        }
    }
}

impl Effect for AddEffect {
    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut result = vec![0.0_f32; self.channel_count];
        for child in &self.children {
            for (r, v) in result.iter_mut().zip(child.tick(ctx)) {
                *r += v;
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// MultiplyEffect – element-wise product of two effects (e.g. envelope × pattern)
// ---------------------------------------------------------------------------

pub struct MultiplyEffect {
    a: Box<dyn Effect>,
    b: Box<dyn Effect>,
    channel_count: usize,
}

impl MultiplyEffect {
    pub fn new(a: Box<dyn Effect>, b: Box<dyn Effect>) -> Self {
        assert_eq!(
            a.channel_count(),
            b.channel_count(),
            "MultiplyEffect: both children must have the same channel count"
        );
        let channel_count = a.channel_count();
        Self { a, b, channel_count }
    }
}

impl Effect for MultiplyEffect {
    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        self.a
            .tick(ctx)
            .into_iter()
            .zip(self.b.tick(ctx))
            .map(|(a, b)| a * b)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// ScaleEffect – multiply all channels by a constant scalar
// ---------------------------------------------------------------------------

pub struct ScaleEffect {
    inner: Box<dyn Effect>,
    scale: f32,
}

impl ScaleEffect {
    pub fn new(inner: Box<dyn Effect>, scale: f32) -> Self {
        Self { inner, scale }
    }
}

impl Effect for ScaleEffect {
    fn channel_count(&self) -> usize {
        self.inner.channel_count()
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        self.inner.tick(ctx).into_iter().map(|v| v * self.scale).collect()
    }
}

// ---------------------------------------------------------------------------
// MaxEffect – element-wise maximum (HTP) of N child effects
// ---------------------------------------------------------------------------

pub struct MaxEffect {
    children: Vec<Box<dyn Effect>>,
    channel_count: usize,
}

impl MaxEffect {
    pub fn new(children: Vec<Box<dyn Effect>>) -> Self {
        assert!(!children.is_empty(), "MaxEffect requires at least one child");
        let channel_count = children[0].channel_count();
        for c in &children {
            assert_eq!(
                c.channel_count(),
                channel_count,
                "MaxEffect: all children must have the same channel count"
            );
        }
        Self {
            children,
            channel_count,
        }
    }
}

impl Effect for MaxEffect {
    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let mut result = vec![f32::NEG_INFINITY; self.channel_count];
        for child in &self.children {
            for (r, v) in result.iter_mut().zip(child.tick(ctx)) {
                *r = r.max(v);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// BlendEffect – linear interpolation between two effects by a fixed factor
// ---------------------------------------------------------------------------

pub struct BlendEffect {
    a: Box<dyn Effect>,
    b: Box<dyn Effect>,
    /// 0.0 = fully `a`, 1.0 = fully `b`.
    factor: f32,
    channel_count: usize,
}

impl BlendEffect {
    pub fn new(a: Box<dyn Effect>, b: Box<dyn Effect>, factor: f32) -> Self {
        assert_eq!(
            a.channel_count(),
            b.channel_count(),
            "BlendEffect: both children must have the same channel count"
        );
        let channel_count = a.channel_count();
        Self { a, b, factor, channel_count }
    }
}

impl Effect for BlendEffect {
    fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn tick(&self, ctx: &TickContext) -> Vec<f32> {
        let t = self.factor;
        self.a
            .tick(ctx)
            .into_iter()
            .zip(self.b.tick(ctx))
            .map(|(va, vb)| va + (vb - va) * t)
            .collect()
    }
}
