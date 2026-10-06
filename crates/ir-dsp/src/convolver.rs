use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ConvolverError {
    #[error("partition size must be a non-zero power of two")]
    InvalidPartitionSize,
    #[error("process block must contain exactly {expected} samples, found {actual}")]
    InvalidBlockSize { expected: usize, actual: usize },
}

/// Uniform overlap-add partitioned FFT convolver. Construction allocates;
/// `process_block` performs no allocation and accepts one fixed quantum.
pub struct PartitionedConvolver {
    partition_size: usize,
    fft_len: usize,
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    ir_partitions: Vec<Vec<Complex32>>,
    input_history: Vec<Vec<Complex32>>,
    history_index: usize,
    fft_buffer: Vec<Complex32>,
    accumulation: Vec<Complex32>,
    forward_scratch: Vec<Complex32>,
    inverse_scratch: Vec<Complex32>,
    overlap: Vec<f32>,
}

impl PartitionedConvolver {
    pub fn new(ir: &[f32], partition_size: usize) -> Result<Self, ConvolverError> {
        if partition_size == 0 || !partition_size.is_power_of_two() {
            return Err(ConvolverError::InvalidPartitionSize);
        }
        let fft_len = partition_size * 2;
        let mut planner = FftPlanner::new();
        let forward = planner.plan_fft_forward(fft_len);
        let inverse = planner.plan_fft_inverse(fft_len);
        let forward_scratch_len = forward.get_inplace_scratch_len();
        let inverse_scratch_len = inverse.get_inplace_scratch_len();
        let partition_count = ir.len().max(1).div_ceil(partition_size);
        let mut ir_partitions = Vec::with_capacity(partition_count);
        for partition in 0..partition_count {
            let mut spectrum = vec![Complex32::ZERO; fft_len];
            let start = partition * partition_size;
            let end = (start + partition_size).min(ir.len());
            for (target, &sample) in spectrum
                .iter_mut()
                .zip(ir.get(start..end).unwrap_or_default())
            {
                target.re = sample;
            }
            forward.process(&mut spectrum);
            ir_partitions.push(spectrum);
        }
        Ok(Self {
            partition_size,
            fft_len,
            forward,
            inverse,
            ir_partitions,
            input_history: vec![vec![Complex32::ZERO; fft_len]; partition_count],
            history_index: 0,
            fft_buffer: vec![Complex32::ZERO; fft_len],
            accumulation: vec![Complex32::ZERO; fft_len],
            forward_scratch: vec![Complex32::ZERO; forward_scratch_len],
            inverse_scratch: vec![Complex32::ZERO; inverse_scratch_len],
            overlap: vec![0.0; partition_size],
        })
    }

    pub fn partition_size(&self) -> usize {
        self.partition_size
    }
    pub fn latency_samples(&self) -> usize {
        0
    }

    pub fn reset(&mut self) {
        for spectrum in &mut self.input_history {
            spectrum.fill(Complex32::ZERO);
        }
        self.overlap.fill(0.0);
        self.history_index = 0;
    }

    pub fn process_block(
        &mut self,
        input: &[f32],
        output: &mut [f32],
    ) -> Result<(), ConvolverError> {
        if input.len() != self.partition_size || output.len() != self.partition_size {
            return Err(ConvolverError::InvalidBlockSize {
                expected: self.partition_size,
                actual: input.len().min(output.len()),
            });
        }
        self.fft_buffer.fill(Complex32::ZERO);
        for (bin, &sample) in self.fft_buffer.iter_mut().zip(input) {
            bin.re = sample;
        }
        self.forward
            .process_with_scratch(&mut self.fft_buffer, &mut self.forward_scratch);
        self.input_history[self.history_index].copy_from_slice(&self.fft_buffer);
        self.accumulation.fill(Complex32::ZERO);
        for partition in 0..self.ir_partitions.len() {
            let history = (self.history_index + self.ir_partitions.len() - partition)
                % self.ir_partitions.len();
            for ((sum, input), ir) in self
                .accumulation
                .iter_mut()
                .zip(&self.input_history[history])
                .zip(&self.ir_partitions[partition])
            {
                *sum += *input * *ir;
            }
        }
        self.inverse
            .process_with_scratch(&mut self.accumulation, &mut self.inverse_scratch);
        let scale = 1.0 / self.fft_len as f32;
        for (index, sample) in output.iter_mut().enumerate() {
            *sample = self.accumulation[index].re * scale + self.overlap[index];
            self.overlap[index] = self.accumulation[index + self.partition_size].re * scale;
        }
        self.history_index = (self.history_index + 1) % self.ir_partitions.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_direct_convolution_across_partitions() {
        let ir = [1.0, 0.5, -0.25, 0.125, 0.0625];
        let input = [0.2, 0.4, -0.3, 0.7, 0.1, -0.2, 0.0, 0.9];
        let mut convolver = PartitionedConvolver::new(&ir, 4).unwrap();
        let mut actual = vec![0.0; input.len() + 8];
        for (block, destination) in input
            .chunks(4)
            .chain(std::iter::repeat_n(&[0.0; 4][..], 2))
            .zip(actual.chunks_mut(4))
        {
            convolver.process_block(block, destination).unwrap();
        }
        let mut expected = vec![0.0; input.len() + ir.len() - 1];
        for (n, &sample) in input.iter().enumerate() {
            for (k, &tap) in ir.iter().enumerate() {
                expected[n + k] += sample * tap;
            }
        }
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1.0e-5, "{a} != {e}");
        }
    }
}
