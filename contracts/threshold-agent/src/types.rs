use soroban_sdk::{contracttype, Address, Bytes, Vec};

#[contracttype]
pub struct ThresholdKeyShare {
    pub agent_id: u64,
    pub share_holder: Address,
    pub share_index: u32,
    pub x_coordinate: u32,             // Shamir’s scheme
    pub y_coordinate_encrypted: Bytes, // Encrypted share
    pub commitment: Bytes,             // Commitment for verification
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProposalStatus {
    Pending,
    Executed,
    Revoked,
}

#[contracttype]
pub struct ThresholdProposal {
    pub proposal_id: u64,
    pub agent_id: u64,
    pub action_data: Bytes,
    pub proposer: Address,
    pub threshold_m: u32,
    pub signatures: Vec<Address>,
    pub status: ProposalStatus,
}
