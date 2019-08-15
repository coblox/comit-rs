use bitcoin::Block;
use std::convert::AsRef;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MinedBlock {
    pub block: Block,
    pub height: u32,
}

impl MinedBlock {
    pub fn new(block: Block, height: u32) -> MinedBlock {
        MinedBlock { block, height }
    }
}

impl AsRef<Block> for MinedBlock {
    fn as_ref(&self) -> &Block {
        &self.block
    }
}
