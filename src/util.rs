use alloy::primitives::{Address, U256, address};
use alloy::providers::Provider;
use alloy::sol;
use polymarket_client_sdk::ContractConfig;

const USDC_ADDRESS: Address = address!("0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174");

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function approve(address spender, uint256 value) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
    }

    #[sol(rpc)]
    interface IERC1155 {
        function setApprovalForAll(address operator, bool approved) external;
        function isApprovedForAll(address account, address operator) external view returns (bool);
    }
}

pub async fn set_all_approvals<P: Provider>(
    provider: &P,
    owner: Address,
    config: &ContractConfig,
    neg_risk_config: &ContractConfig,
    allowance_amount: U256,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = IERC20::new(USDC_ADDRESS, provider.clone());
    let ctf = IERC1155::new(config.conditional_tokens, provider.clone());

    println!("Approving USDC allowances for address: {:?}", owner);
    println!("USDC Contract: {:?}", USDC_ADDRESS);
    println!("ConditionalTokens Contract: {:?}", config.conditional_tokens);
    println!("Exchange Contract: {:?}", config.exchange);
    println!("NegRisk Exchange Contract: {:?}", neg_risk_config.exchange);
    
    let balance = token.balanceOf(owner).call().await?;
    let decimals = token.decimals().call().await?;
    
    let balance_formatted = format_balance(balance, decimals);
    println!("\n💰 USDC Balance: {} USDC", balance_formatted);
    println!("📊 Available for trading: {} USDC\n", balance_formatted);

    let ctf_allowance = token.allowance(owner, config.conditional_tokens).call().await?;
    if ctf_allowance < allowance_amount {
        println!("Current CTF allowance: {}, setting to {}...", ctf_allowance, allowance_amount);
        approve(&token, config.conditional_tokens, allowance_amount).await?;
        println!("✅ USDC approved for ConditionalTokens contract");
    } else {
        println!("✅ USDC already approved for ConditionalTokens contract ({})", ctf_allowance);
    }

    let is_ctf_approved = ctf.isApprovedForAll(owner, config.conditional_tokens).call().await?;
    if !is_ctf_approved {
        println!("Approving ConditionalTokens for ConditionalTokens contract...");
        set_approval_for_all(&ctf, config.conditional_tokens, true).await?;
        println!("✅ ConditionalTokens approved for ConditionalTokens contract");
    } else {
        println!("✅ ConditionalTokens already approved for ConditionalTokens contract");
    }

    let exchange_allowance = token.allowance(owner, config.exchange).call().await?;
    if exchange_allowance < allowance_amount {
        println!("Current Exchange allowance: {}, setting to {}...", exchange_allowance, allowance_amount);
        approve(&token, config.exchange, allowance_amount).await?;
        println!("✅ USDC approved for Exchange contract");
    } else {
        println!("✅ USDC already approved for Exchange contract ({})", exchange_allowance);
    }

    let is_exchange_approved = ctf.isApprovedForAll(owner, config.exchange).call().await?;
    if !is_exchange_approved {
        println!("Approving ConditionalTokens for Exchange contract...");
        set_approval_for_all(&ctf, config.exchange, true).await?;
        println!("✅ ConditionalTokens approved for Exchange contract");
    } else {
        println!("✅ ConditionalTokens already approved for Exchange contract");
    }

    let neg_risk_allowance = token.allowance(owner, neg_risk_config.exchange).call().await?;
    if neg_risk_allowance < allowance_amount {
        println!("Current NegRisk Exchange allowance: {}, setting to {}...", neg_risk_allowance, allowance_amount);
        approve(&token, neg_risk_config.exchange, allowance_amount).await?;
        println!("✅ USDC approved for NegRisk Exchange contract");
    } else {
        println!("✅ USDC already approved for NegRisk Exchange contract ({})", neg_risk_allowance);
    }

    let is_neg_risk_approved = ctf.isApprovedForAll(owner, neg_risk_config.exchange).call().await?;
    if !is_neg_risk_approved {
        println!("Approving ConditionalTokens for NegRisk Exchange contract...");
        set_approval_for_all(&ctf, neg_risk_config.exchange, true).await?;
        println!("✅ ConditionalTokens approved for NegRisk Exchange contract");
    } else {
        println!("✅ ConditionalTokens already approved for NegRisk Exchange contract");
    }

    println!("All allowances approved successfully!");

    let final_balance = token.balanceOf(owner).call().await?;
    let final_balance_formatted = format_balance(final_balance, decimals);
    println!("\n💰 Final USDC Balance: {} USDC", final_balance_formatted);
    println!("📊 Available for trading: {} USDC\n", final_balance_formatted);
    
    Ok(())
}

fn format_balance(balance: U256, decimals: u8) -> String {
    let balance_str = balance.to_string();
    let decimals_usize = decimals as usize;
    
    if balance_str.len() <= decimals_usize {
        let padded = format!("{:0>width$}", balance_str, width = decimals_usize);
        let trimmed = padded.trim_end_matches('0');
        if trimmed.is_empty() {
            "0".to_string()
        } else {
            format!("0.{}", trimmed)
        }
    } else {
        let split_point = balance_str.len() - decimals_usize;
        let whole = &balance_str[..split_point];
        let fractional = &balance_str[split_point..];
        let fractional_trimmed = fractional.trim_end_matches('0');
        
        if fractional_trimmed.is_empty() {
            whole.to_string()
        } else {
            format!("{}.{}", whole, fractional_trimmed)
        }
    }
}

async fn approve<P: Provider>(
    usdc: &IERC20::IERC20Instance<P>,
    spender: Address,
    amount: U256,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Calling USDC.approve({spender:?}, {amount})...");

    let receipt = usdc.approve(spender, amount).send().await?.watch().await?;

    println!("USDC approve tx mined: {receipt:?}");

    Ok(())
}

async fn set_approval_for_all<P: Provider>(
    ctf: &IERC1155::IERC1155Instance<P>,
    operator: Address,
    approved: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Calling CTF.setApprovalForAll({operator:?}, {approved})...");

    let receipt = ctf
        .setApprovalForAll(operator, approved)
        .send()
        .await?
        .watch()
        .await?;

    println!("CTF setApprovalForAll tx mined: {receipt:?}");

    Ok(())
}
