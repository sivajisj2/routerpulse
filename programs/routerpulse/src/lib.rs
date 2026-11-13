use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hashv;

declare_id!("4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi");

pub const MAX_PAYLOAD_BYTES: usize = 1024;
pub const MESSAGE_DOMAIN: &[u8] = b"ROUTERPULSE_MESSAGE_V1";

#[program]
pub mod routerpulse {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, local_domain_hash: [u8; 32], relayer: Pubkey) -> Result<()> {
        require_keys_eq!(ctx.accounts.authority.key(), ctx.accounts.payer.key(), RouterPulseError::InvalidAuthority);
        require_keys_neq!(relayer, Pubkey::default(), RouterPulseError::InvalidRelayer);
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.relayer = relayer;
        config.local_domain_hash = local_domain_hash;
        config.paused = false;
        config.bump = ctx.bumps.config;
        emit!(ProtocolInitialized { authority: config.authority, relayer, local_domain_hash });
        Ok(())
    }

    pub fn register_remote(
        ctx: Context<RegisterRemote>,
        remote_domain_hash: [u8; 32],
        remote_program: Pubkey,
        enabled: bool,
    ) -> Result<()> {
        require_keys_neq!(remote_program, Pubkey::default(), RouterPulseError::InvalidRemote);
        require!(remote_domain_hash != ctx.accounts.config.local_domain_hash, RouterPulseError::InvalidRemote);
        let remote = &mut ctx.accounts.remote;
        remote.domain_hash = remote_domain_hash;
        remote.remote_program = remote_program;
        remote.enabled = enabled;
        remote.bump = ctx.bumps.remote;
        emit!(RemoteUpdated { remote_domain_hash, remote_program, enabled });
        Ok(())
    }

    pub fn update_remote(ctx: Context<UpdateRemote>, enabled: bool) -> Result<()> {
        let remote = &mut ctx.accounts.remote;
        remote.enabled = enabled;
        emit!(RemoteUpdated {
            remote_domain_hash: remote.domain_hash,
            remote_program: remote.remote_program,
            enabled,
        });
        Ok(())
    }

    pub fn set_relayer(ctx: Context<AdminConfig>, relayer: Pubkey) -> Result<()> {
        require_keys_neq!(relayer, Pubkey::default(), RouterPulseError::InvalidRelayer);
        ctx.accounts.config.relayer = relayer;
        emit!(RelayerUpdated { relayer });
        Ok(())
    }

    pub fn set_paused(ctx: Context<AdminConfig>, paused: bool) -> Result<()> {
        ctx.accounts.config.paused = paused;
        emit!(PauseUpdated { paused });
        Ok(())
    }

    pub fn send_message(
        ctx: Context<SendMessage>,
        destination_domain_hash: [u8; 32],
        destination_program: Pubkey,
        receiver: Pubkey,
        payload: Vec<u8>,
        expires_at: i64,
    ) -> Result<()> {
        require!(!ctx.accounts.config.paused, RouterPulseError::Paused);
        require!(ctx.accounts.remote.enabled, RouterPulseError::RemoteDisabled);
        require!(payload.len() <= MAX_PAYLOAD_BYTES, RouterPulseError::PayloadTooLarge);
        require_keys_neq!(receiver, Pubkey::default(), RouterPulseError::InvalidReceiver);
        let clock = Clock::get()?;
        require!(expires_at > clock.unix_timestamp, RouterPulseError::MessageExpired);
        require!(expires_at <= clock.unix_timestamp + 30 * 24 * 60 * 60, RouterPulseError::ExpiryTooFar);
        require!(ctx.accounts.remote.domain_hash == destination_domain_hash, RouterPulseError::InvalidRemote);
        require_keys_eq!(ctx.accounts.remote.remote_program, destination_program, RouterPulseError::InvalidRemote);

        let sender = ctx.accounts.sender.key();
        let sequence = ctx.accounts.sender_sequence.next_sequence;
        let payload_hash = hashv(&[&payload]).to_bytes();
        let message_id = canonical_message_id(
            ctx.accounts.config.local_domain_hash,
            crate::ID,
            destination_domain_hash,
            destination_program,
            sender,
            sequence,
            receiver,
            payload_hash,
            expires_at,
        );

        let message = &mut ctx.accounts.message;
        message.message_id = message_id;
        message.source_domain_hash = ctx.accounts.config.local_domain_hash;
        message.destination_domain_hash = destination_domain_hash;
        message.source_program = crate::ID;
        message.destination_program = destination_program;
        message.sender = sender;
        message.receiver = receiver;
        message.sequence = sequence;
        message.payload_hash = payload_hash;
        message.payload = payload;
        message.expires_at = expires_at;
        message.created_at = clock.unix_timestamp;
        message.bump = ctx.bumps.message;

        ctx.accounts.sender_sequence.next_sequence = sequence.checked_add(1).ok_or(RouterPulseError::SequenceOverflow)?;
        emit!(MessageSent {
            message_id,
            source_domain_hash: message.source_domain_hash,
            destination_domain_hash,
            source_program: crate::ID,
            destination_program,
            sender,
            receiver,
            sequence,
            payload_hash,
            expires_at,
        });
        Ok(())
    }

    /// Records destination execution after a configured relayer attests to a
    /// finalized observation on another Solana cluster. This is permissioned,
    /// not a trustless bridge proof. It intentionally does not invoke arbitrary
    /// receiver programs or move tokens.
    pub fn execute_attested_message(
        ctx: Context<ExecuteAttestedMessage>,
        source_domain_hash: [u8; 32],
        source_program: Pubkey,
        sender: Pubkey,
        sequence: u64,
        receiver: Pubkey,
        payload_hash: [u8; 32],
        expires_at: i64,
        message_id: [u8; 32],
    ) -> Result<()> {
        require!(!ctx.accounts.config.paused, RouterPulseError::Paused);
        require!(ctx.accounts.remote.enabled, RouterPulseError::RemoteDisabled);
        require!(ctx.accounts.remote.domain_hash == source_domain_hash, RouterPulseError::InvalidRemote);
        require_keys_eq!(ctx.accounts.remote.remote_program, source_program, RouterPulseError::InvalidRemote);
        require_keys_eq!(ctx.accounts.relayer.key(), ctx.accounts.config.relayer, RouterPulseError::UnauthorizedRelayer);
        require!(expires_at > Clock::get()?.unix_timestamp, RouterPulseError::MessageExpired);
        require_keys_eq!(ctx.accounts.receiver.key(), receiver, RouterPulseError::InvalidReceiver);

        let expected_id = canonical_message_id(
            source_domain_hash,
            source_program,
            ctx.accounts.config.local_domain_hash,
            crate::ID,
            sender,
            sequence,
            receiver,
            payload_hash,
            expires_at,
        );
        require!(expected_id == message_id, RouterPulseError::InvalidMessageId);

        let processed = &mut ctx.accounts.processed;
        require!(!processed.executed, RouterPulseError::AlreadyProcessed);
        processed.message_id = message_id;
        processed.source_domain_hash = source_domain_hash;
        processed.source_program = source_program;
        processed.executed_at = Clock::get()?.unix_timestamp;
        processed.executed = true;
        processed.bump = ctx.bumps.processed;
        emit!(MessageExecuted { message_id, receiver, relayer: ctx.accounts.relayer.key(), executed_at: processed.executed_at });
        Ok(())
    }
}

fn canonical_message_id(
    source_domain_hash: [u8; 32], source_program: Pubkey,
    destination_domain_hash: [u8; 32], destination_program: Pubkey,
    sender: Pubkey, sequence: u64, receiver: Pubkey,
    payload_hash: [u8; 32], expires_at: i64,
) -> [u8; 32] {
    hashv(&[
        MESSAGE_DOMAIN,
        &source_domain_hash,
        source_program.as_ref(),
        &destination_domain_hash,
        destination_program.as_ref(),
        sender.as_ref(),
        &sequence.to_le_bytes(),
        receiver.as_ref(),
        &payload_hash,
        &expires_at.to_le_bytes(),
    ]).to_bytes()
}

#[derive(Accounts)]
#[instruction(local_domain_hash: [u8; 32])]
pub struct Initialize<'info> {
    #[account(mut)] pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    #[account(init, payer = payer, space = 8 + RouterConfig::INIT_SPACE, seeds = [b"config"], bump)]
    pub config: Account<'info, RouterConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(remote_domain_hash: [u8; 32], remote_program: Pubkey)]
pub struct RegisterRemote<'info> {
    #[account(mut)] pub authority: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump, has_one = authority)] pub config: Account<'info, RouterConfig>,
    #[account(init, payer = authority, space = 8 + RemoteConfig::INIT_SPACE, seeds = [b"remote", remote_domain_hash.as_ref(), remote_program.as_ref()], bump)]
    pub remote: Account<'info, RemoteConfig>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(remote_domain_hash: [u8; 32], remote_program: Pubkey)]
pub struct UpdateRemote<'info> {
    pub authority: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump, has_one = authority)]
    pub config: Account<'info, RouterConfig>,
    #[account(mut, seeds = [b"remote", remote_domain_hash.as_ref(), remote_program.as_ref()], bump = remote.bump)]
    pub remote: Account<'info, RemoteConfig>,
}

#[derive(Accounts)]
pub struct AdminConfig<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump, has_one = authority)] pub config: Account<'info, RouterConfig>,
}

#[derive(Accounts)]
#[instruction(destination_domain_hash: [u8; 32], destination_program: Pubkey, receiver: Pubkey, payload: Vec<u8>, expires_at: i64)]
pub struct SendMessage<'info> {
    #[account(mut)] pub sender: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)] pub config: Account<'info, RouterConfig>,
    #[account(seeds = [b"remote", destination_domain_hash.as_ref(), destination_program.as_ref()], bump = remote.bump)] pub remote: Account<'info, RemoteConfig>,
    #[account(init_if_needed, payer = sender, space = 8 + SenderSequence::INIT_SPACE, seeds = [b"sender", sender.key().as_ref()], bump)] pub sender_sequence: Account<'info, SenderSequence>,
    #[account(init, payer = sender, space = 8 + MessageAccount::INIT_SPACE, seeds = [b"message", sender.key().as_ref(), &sender_sequence.next_sequence.to_le_bytes()], bump)] pub message: Account<'info, MessageAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(source_domain_hash: [u8; 32], source_program: Pubkey, sender: Pubkey, sequence: u64, receiver: Pubkey, payload_hash: [u8; 32], expires_at: i64, message_id: [u8; 32])]
pub struct ExecuteAttestedMessage<'info> {
    #[account(mut)] pub relayer: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)] pub config: Account<'info, RouterConfig>,
    #[account(seeds = [b"remote", source_domain_hash.as_ref(), source_program.as_ref()], bump = remote.bump)] pub remote: Account<'info, RemoteConfig>,
    /// CHECK: Address equality with the message receiver is enforced in the handler.
    pub receiver: UncheckedAccount<'info>,
    #[account(init_if_needed, payer = relayer, space = 8 + ProcessedMessage::INIT_SPACE, seeds = [b"processed", message_id.as_ref()], bump)] pub processed: Account<'info, ProcessedMessage>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct RouterConfig {
    pub authority: Pubkey,
    pub relayer: Pubkey,
    pub local_domain_hash: [u8; 32],
    pub paused: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct RemoteConfig {
    pub domain_hash: [u8; 32],
    pub remote_program: Pubkey,
    pub enabled: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct SenderSequence { pub next_sequence: u64 }

#[account]
#[derive(InitSpace)]
pub struct MessageAccount {
    pub message_id: [u8; 32],
    pub source_domain_hash: [u8; 32],
    pub destination_domain_hash: [u8; 32],
    pub source_program: Pubkey,
    pub destination_program: Pubkey,
    pub sender: Pubkey,
    pub receiver: Pubkey,
    pub sequence: u64,
    pub payload_hash: [u8; 32],
    #[max_len(1024)] pub payload: Vec<u8>,
    pub expires_at: i64,
    pub created_at: i64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct ProcessedMessage {
    pub message_id: [u8; 32],
    pub source_domain_hash: [u8; 32],
    pub source_program: Pubkey,
    pub executed_at: i64,
    pub executed: bool,
    pub bump: u8,
}

#[event]
pub struct ProtocolInitialized { pub authority: Pubkey, pub relayer: Pubkey, pub local_domain_hash: [u8; 32] }
#[event]
pub struct RemoteUpdated { pub remote_domain_hash: [u8; 32], pub remote_program: Pubkey, pub enabled: bool }
#[event]
pub struct RelayerUpdated { pub relayer: Pubkey }
#[event]
pub struct PauseUpdated { pub paused: bool }
#[event]
pub struct MessageSent { pub message_id: [u8; 32], pub source_domain_hash: [u8; 32], pub destination_domain_hash: [u8; 32], pub source_program: Pubkey, pub destination_program: Pubkey, pub sender: Pubkey, pub receiver: Pubkey, pub sequence: u64, pub payload_hash: [u8; 32], pub expires_at: i64 }
#[event]
pub struct MessageExecuted { pub message_id: [u8; 32], pub receiver: Pubkey, pub relayer: Pubkey, pub executed_at: i64 }

#[error_code]
pub enum RouterPulseError {
    #[msg("Authority signer does not match the configured authority.")] InvalidAuthority,
    #[msg("Relayer pubkey cannot be the default address.")] InvalidRelayer,
    #[msg("Remote domain or program is invalid.")] InvalidRemote,
    #[msg("Remote route is disabled.")] RemoteDisabled,
    #[msg("Program is paused.")] Paused,
    #[msg("Payload exceeds the maximum supported size.")] PayloadTooLarge,
    #[msg("Receiver cannot be the default address.")] InvalidReceiver,
    #[msg("Message expiration must be in the future.")] MessageExpired,
    #[msg("Message expiration cannot exceed 30 days.")] ExpiryTooFar,
    #[msg("Only the configured relayer may attest to a remote message.")] UnauthorizedRelayer,
    #[msg("Canonical message ID does not match the supplied fields.")] InvalidMessageId,
    #[msg("This message has already been executed.")] AlreadyProcessed,
    #[msg("Sender sequence overflow.")] SequenceOverflow,
}
