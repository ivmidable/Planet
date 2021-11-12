#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, from_binary, Binary, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128, Addr
};

//for tests
use std::time::{SystemTime, UNIX_EPOCH};

use cw20_base::allowances::{
    execute_burn_from, execute_decrease_allowance, execute_increase_allowance, execute_send_from,
    execute_transfer_from, query_allowance,
};
use cw20_base::contract::{
    execute_burn, execute_mint, execute_send, execute_transfer, query_balance, query_token_info,
};

use cw20_base::state::{MinterData, TokenInfo, TOKEN_INFO};

use cw2::set_contract_version;

use crate::error::ContractError;

use crate::msg::AskForPlanetResponse;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Planet, PLANET};

use sha2::Digest;

// version info for migration info
const CONTRACT_NAME: &str = "Planet";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let auth = deps.api.addr_validate(&info.sender.clone().into_string())?;

    if auth.into_string() != "terra1dcegyrekltswvyy0xy69ydgxn9x8x32zdtapd8" {
        return Err(cosmwasm_std::StdError::GenericErr{msg:"Invalid Authority".into()});
    }

    // store token info using cw20-base format
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply: msg.total_supply,//Uint128::zero(),
        // set self as minter, so we can properly execute mint and burn
        mint: Some(MinterData {
            minter: env.contract.address.clone(),
            cap: None,
        }),
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    //REMOVE AFTER TESTING
    /*let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).unwrap();
    let time = since_the_epoch.as_secs();*/

    let mut data_vec = env.contract.address.as_bytes().to_vec();
    data_vec.extend_from_slice(&env.block.time.nanos().to_le_bytes());
    //data_vec.extend_from_slice(&time.to_le_bytes());
    data_vec.extend_from_slice(&env.block.height.to_le_bytes());

    let planet = Planet {
        epoch: 0,
        epoch_start_block: env.block.height.clone(),
        total_mined: 0,
        mined_this_epoch: 0,
        hash: sha2::Sha256::digest(data_vec.as_slice()).into(),
        diff: 2,
        tokens: 100000000000,
    };

    PLANET.save(deps.storage, &planet)?;

    let mint_info = MessageInfo {
        sender: env.contract.address.clone(),
        funds: vec![],
    };
    let t:u64 = 100000000000;
    execute_mint(deps, env, mint_info, info.sender.to_string(), Uint128::from(t)).unwrap();
    
    Ok(Response::new().add_attribute("action", "instantiate") )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Claim {nonce} => execute_claim(deps, env, info, nonce),// these all come from cw20-base to implement the cw20 standard
        ExecuteMsg::Transfer { recipient, amount } => {
            Ok(execute_transfer(deps, env, info, recipient, amount)?)
        }
        ExecuteMsg::Burn { amount } => Ok(execute_burn(deps, env, info, amount)?),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => Ok(execute_send(deps, env, info, contract, amount, msg)?),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => Ok(execute_increase_allowance(
            deps, env, info, spender, amount, expires,
        )?),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => Ok(execute_decrease_allowance(
            deps, env, info, spender, amount, expires,
        )?),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => Ok(execute_transfer_from(
            deps, env, info, owner, recipient, amount,
        )?),
        ExecuteMsg::BurnFrom { owner, amount } => {
            Ok(execute_burn_from(deps, env, info, owner, amount)?)
        }
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => Ok(execute_send_from(
            deps, env, info, owner, contract, amount, msg,
        )?),
    }
}

pub fn execute_claim(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    nonce:[u8;32]
) -> Result<Response, ContractError> {
    let mut work = PLANET.load(deps.storage)?;
    
    if check_claim(info.sender.as_bytes(), &nonce, &work.hash, &work.diff) == false {
        return Err(ContractError::InvalidClaim {});
    }

    let mut data_vec = env.contract.address.as_bytes().to_vec();
    data_vec.extend_from_slice(&env.block.time.nanos().to_le_bytes());
    data_vec.extend_from_slice(&env.block.height.to_le_bytes());
    data_vec.extend_from_slice(&work.hash[..]);
    data_vec.extend_from_slice(&work.total_mined.to_le_bytes());

    work.hash = sha2::Sha256::digest(data_vec.as_slice()).into();

    work.total_mined += 1;

    if env.block.height - work.epoch_start_block > 5000 {
        if work.mined_this_epoch >= 1000 {
            work.diff += 1;
        }
        work.mined_this_epoch = 0;
        work.epoch_start_block = env.block.height;
    }

    PLANET.save(deps.storage, &work)?;

    let mint_info = MessageInfo {
        sender: env.contract.address.clone(),
        funds: vec![],
    };
    execute_mint(deps, env, mint_info, info.sender.to_string(), Uint128::from(work.tokens))?;

    Ok(Response::new().add_attribute("action", "claim"))
}

pub fn check_claim(
    sender: &[u8],
    nonce: &[u8; 32],
    sha256: &[u8; 32],
    diff: &u8,
) -> bool {
    let mut magic_raw: [u8; 32] = [0; 32];
    magic_raw[0] = 33;
    magic_raw[1] = 232;
    let (magic, _rest) = magic_raw.split_at(*diff as usize);
    let mut data_vec = sha256.to_vec();
    data_vec.extend_from_slice(sender);
    data_vec.extend_from_slice(&nonce[..]);
    let hash_vec = sha2::Sha256::digest(data_vec.as_slice()).to_vec();    
    if hash_vec.starts_with(magic) == false {
        return false;
    }
    return true;
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    //Cw721HeroContract::default().query(deps, env, msg)
    match msg {
        QueryMsg::Planet {} => to_binary(&query_current_work(deps)?),
        // inherited from cw20-base
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
    }
}

fn query_current_work(deps: Deps) -> StdResult<AskForPlanetResponse> {
    let planet = PLANET.load(deps.storage)?;
    //let ask = TOKEN_ASKS.load(deps.storage, (&"mint", &"mint"))?;
    Ok(AskForPlanetResponse { planet })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coin, Uint128};

    const INITER: &str = "terra1dcegyrekltswvyy0xy69ydgxn9x8x32zdtapd8";

    fn setup_contract(deps: DepsMut) {
        let msg = InstantiateMsg {
            name: "Pluto".into(),
            symbol: "PLT".into(),
            decimals: 9,
            total_supply: Uint128::zero(),
        };
        let info = mock_info(INITER, &[]);
        let res = instantiate(deps, mock_env(), info, msg).unwrap();
        //console.log(res);
        assert_eq!(0, res.messages.len());
    }

    fn claim(deps: DepsMut) {
        let info = mock_info(INITER, &[]);
        
        
        let work = QueryMsg::Planet {};
        let res:AskForPlanetResponse = from_binary(&query(deps.as_ref(), mock_env(), work).unwrap()).unwrap();

        //println!("{:?}", );
        let mut nonce:[u8; 32] = sha2::Sha256::digest(b"start").into();

        //let mut count:u64 = 0;
        loop {
            let mut data_vec = vec![info.sender.as_bytes()];
            data_vec.extend_from_slice(&[&nonce[..]]);
            if check_claim(info.sender.as_bytes(), &nonce, &res.planet.hash, &res.planet.diff) == true {
                break;
            }
            nonce = sha2::Sha256::digest(&nonce[..]).into();
            //count = count.checked_add(1).unwrap();
        }

        let claim_msg = ExecuteMsg::Claim {
            nonce:nonce
        };


        let _ = execute(deps, mock_env(), info, claim_msg).unwrap();

        //println!("{}", count);

       // assert_eq!(1, 2);
    }


    #[test]
    fn _0_instantiation() {
        let mut deps = mock_dependencies(&[]);
        setup_contract(deps.as_mut());
    }

    #[test]
    fn _1_claim() {
        let mut deps = mock_dependencies(&[]);
        setup_contract(deps.as_mut());
        claim(deps.as_mut());
    }
}

