use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("<id>");

#[program]
pub mod simple_escrow {
    use super::*;

    pub fn deposit(
        ctx: Context<Deposit>,
        amount: u64,
        id: String,
        asset_type: u8, // 0 = SOL, 1 = SPL token (fungible), 2 = NFT
    ) -> Result<()> {
        let escrow = &mut ctx.accounts.escrow_account;
        escrow.id = id.clone();
        escrow.owner = ctx.accounts.user.key();
        escrow.amount = amount;
        escrow.asset_type = asset_type;
        escrow.token_mint = ctx.accounts.token_mint.key();

        if asset_type == 0 {
            // SOL deposit
            let escrow_pda = ctx.accounts.escrow_pda.to_account_info();
            **escrow_pda.try_borrow_mut_lamports()? += amount;
            **ctx.accounts.user.try_borrow_mut_lamports()? -= amount;
        } else {
            // SPL token or NFT
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx
                            .accounts
                            .user_token_account
                            .as_ref()
                            .unwrap()
                            .to_account_info(),
                        to: ctx
                            .accounts
                            .escrow_token_account
                            .as_ref()
                            .unwrap()
                            .to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                amount,
            )?;
        }

        Ok(())
    }

    pub fn release(ctx: Context<Release>, id: String) -> Result<()> {
        let escrow = &ctx.accounts.escrow_account;
        require!(escrow.id == id, EscrowError::InvalidEscrowId);

        let (escrow_pda_key, bump) =
            Pubkey::find_program_address(&[b"escrow", id.as_bytes()], ctx.program_id);

        require_keys_eq!(
            ctx.accounts.escrow_signer.key(),
            escrow_pda_key,
            EscrowError::Unauthorized
        );

        if escrow.asset_type == 0 {
            // SOL release
            let escrow_pda = ctx.accounts.escrow_pda.to_account_info();
            **escrow_pda.try_borrow_mut_lamports()? -= escrow.amount;
            **ctx.accounts.destination.try_borrow_mut_lamports()? += escrow.amount;
        } else {
            // SPL token or NFT release
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx
                            .accounts
                            .escrow_token_account
                            .as_ref()
                            .unwrap()
                            .to_account_info(),
                        to: ctx
                            .accounts
                            .destination_token_account
                            .as_ref()
                            .unwrap()
                            .to_account_info(),
                        authority: ctx.accounts.escrow_signer.to_account_info(),
                    },
                )
                .with_signer(&[&[b"escrow", id.as_bytes(), &[bump]]]),
                escrow.amount,
            )?;
        }

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(id: String)]
pub struct Deposit<'info> {
    #[account(init, payer = user, space = 8 + 32 + 8 + 32 + 1 + 64)]
    pub escrow_account: Account<'info, EscrowAccount>,
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub user_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,

    /// CHECK: PDA to hold SOL (only used for lamports)
    #[account(seeds = [b"escrow", id.as_bytes()], bump)]
    pub escrow_pda: AccountInfo<'info>,

    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
#[instruction(id: String)]
pub struct Release<'info> {
    #[account(mut)]
    pub escrow_account: Account<'info, EscrowAccount>,

    #[account(mut)]
    pub escrow_token_account: Option<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub destination_token_account: Option<Account<'info, TokenAccount>>,

    /// CHECK: Receiver of SOL or token
    #[account(mut)]
    pub destination: AccountInfo<'info>,

    /// CHECK: PDA
    #[account(seeds = [b"escrow", id.as_bytes()], bump)]
    pub escrow_signer: AccountInfo<'info>,

    /// CHECK: For SOL transfer
    #[account(mut)]
    pub escrow_pda: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}

#[account]
pub struct EscrowAccount {
    pub id: String,
    pub owner: Pubkey,
    pub amount: u64,
    pub token_mint: Pubkey,
    pub asset_type: u8, // 0 = SOL, 1 = token, 2 = NFT
}

#[error_code]
pub enum EscrowError {
    #[msg("Unauthorized access.")]
    Unauthorized,
    #[msg("Invalid escrow ID.")]
    InvalidEscrowId,
}
