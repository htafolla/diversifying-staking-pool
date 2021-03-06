#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use near_sdk::{
  borsh::{self, BorshDeserialize, BorshSerialize},
  json_types::{U128, Base58PublicKey},
  serde::{Deserialize, Serialize},
  serde_json::json,
  serde_json::Value,
  *,
};
use near_sdk_sim::{
  account::AccessKey, call, deploy, init_simulator, 
  near_crypto::{Signer,SecretKey, KeyType},
  to_yocto, view,
  ContractAccount, ExecutionResult, UserAccount, DEFAULT_GAS, STORAGE_AMOUNT,
  ViewResult
};


// //Note: the struct xxxxxxContract is created by #[near_bindgen] (near_skd_rs~2.0.4)
use divpool::*;

// Load contracts' bytes.
near_sdk_sim::lazy_static! {
  static ref WASM_BYTES_DIV_POOL: &'static [u8] = include_bytes!("../../res/divpool.wasm").as_ref();
  static ref WASM_BYTES_SP: &'static [u8] = include_bytes!("../../res/staking_pool.wasm").as_ref();
  static ref WASM_BYTES_GET_EPOCH: &'static [u8] = include_bytes!("../../res/get_epoch_contract.wasm").as_ref();
}

const TGAS: u64 = 1_000_000_000_000;
const NEAR: u128 = 1_000_000_000_000_000_000_000_000;
const E24: u128 = NEAR;

const SP_INITIAL_BALANCE:u128 = 100*NEAR;

/// Deploy the contract(s) and create some divpool accounts. Returns:
/// - The divpool Contract
/// - Root Account
/// - Testnet Account (utility suffix for building other addresses)
/// - A deployer account address
fn init_simulator_and_contract(
  initial_balance: u128,
  deploy_to: &str,
) -> (
  ContractAccount<DiversifiedPoolContract>,
  UserAccount, // root
  UserAccount, // testnet suffix
  UserAccount, // deployer account
  UserAccount,
  UserAccount
) {
  // Root account has address: "root"
  let master_account = init_simulator(None);

  // Other accounts may be created from the root account
  // Note: address naming is fully expressive: we may create any suffix we desire, ie testnet, near, etc.
  // but only those two (.testnet, .near) will be used in practice.
  let testnet = master_account.create_user("testnet".to_string(), ntoy(1_000_000_000));

  // We need an account to deploy the contracts from. We may create subaccounts of "testnet" as follows:
  let owner = testnet.create_user(deploy_to.to_string(), ntoy(1_000_000));

  let treasury = testnet.create_user("treasury".to_string(), ntoy(1_000_000));
  let operator = testnet.create_user("operator".to_string(), ntoy(1_000_000));

  let divpool_contract = deploy!(
      contract: DiversifiedPoolContract,
      contract_id: "divpool",
      bytes: &WASM_BYTES_DIV_POOL,
      // User deploying the contract
      signer_account: owner,
      // DiversifiedPool.new(
        //   owner_account_id: AccountId,
        //   treasury_account_id: AccountId,
        //   operator_account_id: AccountId,
      deposit:500*NEAR,
      gas:25*TGAS,
      init_method:new(owner.account_id(), treasury.account_id(), operator.account_id())
      );

  return (divpool_contract, master_account, testnet, owner, treasury, operator)
}

//----------------------
fn view(contract_account: &UserAccount, method:&str, args_json:&str) -> Value {
    let pct = PendingContractTx {
      receiver_id: contract_account.account_id(),
      method: method.into(),
      args: args_json.into(),
      is_view:true,
    };
    let vr = &contract_account.view(pct);
    //println!("view Result: {:#?}", vr.unwrap_json_value());
    return vr.unwrap_json_value();
}
fn as_u128(v:&Value) -> u128 {
  return match v.as_str() {
    Some(x) => {
      //println!("{}",x); 
      x.parse::<u128>().unwrap()
    },
    _ => panic!("invalid u128 value {:#?}", v)
  };
}
fn view_u128 (contract_account: &UserAccount, method:&str, args_json:&str) -> u128 {
  let result = view(contract_account,method,args_json);
  return as_u128(&result)
}

//----------------------
fn call(who: &UserAccount, contract_account: &UserAccount, method:&str, args_json:&str, attached_deposit:u128, gas:u64) -> ExecutionResult {
  let pct = PendingContractTx {
    receiver_id: contract_account.account_id(),
    method: method.into(),
    args: args_json.into(),
    is_view:false,
  };
  let exec_res = who.call(pct,attached_deposit,gas);
  //println!("Result: {:#?}", exec_res);
  return exec_res;
}

//-----------------------
fn deploy_simulated_staking_pool(
    master_account: &UserAccount,
    deploy_to_acc_id: &str,
    owner_account_id: &str,
) 
  -> UserAccount 
{
  let sp = master_account.deploy(&WASM_BYTES_SP, deploy_to_acc_id.into(), SP_INITIAL_BALANCE);
  let user_txn = master_account
    .create_transaction(sp.account_id())
    .function_call(
      "new".into(), 
      format!(r#"{{"owner_id":"{}", "stake_public_key":"Di8H4S8HSwSdwGABTGfKcxf1HaVzWSUKVH1mYQgwHCWb","reward_fee_fraction":{{"numerator":5,"denominator":100}}}}"#,
        owner_account_id
        ).into(),//arguments: Vec<u8>,
      50*TGAS, 0);
  let res = user_txn.submit();
  //print_helper(res);
  return sp;
}

/// Helper to log ExecutionResult outcome of a call/view
fn print_helper(res: &ExecutionResult) {
  println!("Result: {:#?}", res);
  assert!(res.is_ok());
}
fn print_helper_promise(res: &ExecutionResult) {
  println!("Result: {:#?}", res);
  //println!("Receipt results: {:#?}", res.get_receipt_results());
  println!("Promise results: {:#?}", res.promise_results());
  assert!(res.is_ok());
}
/// Helper to log ExecutionResult outcome of a call/view
fn print_helper_profile(res: &ExecutionResult) {
  println!("Promise results: {:#?}", res.promise_results());
  //println!("Receipt results: {:#?}", res.get_receipt_results());
  println!("Profiling: {:#?}", res.profile_data());
  //println!("Result: {:#?}", res);
  assert!(res.is_ok());
}

 fn print_vecu8(title:&str, v:&Vec<u8>){
  println!("{}:{}", title,
   match std::str::from_utf8(v) {
     Ok(v) => v,
     Err(e) => "[[can't decode result, invalid UFT8 sequence]]"
   })
 }

fn ntoy(near:u64) -> u128 { to_yocto(&near.to_string()) }

fn yton(yoctos:u128) -> String { 
  let mut str = yoctos.to_string();
  let dec = str.split_off(str.len()-24);
  return [&str,".",&dec].concat();
}

struct Simulation {
  pub divpool: ContractAccount<DiversifiedPoolContract>,
  pub master_account:UserAccount, // root
  pub testnet:UserAccount, // testnet suffix
  pub owner:UserAccount, // deployer account
  pub treasury:UserAccount,
  pub operator:UserAccount,
  pub sp: Vec<UserAccount> //Staking pools
}

//-----------------------------
//-----------------------------
//-----------------------------
impl Simulation {

  pub fn new() -> Self {

    // Root account has address: "root"
    let master_account = init_simulator(None);
    // Other accounts may be created from the root account
    // Note: address naming is fully expressive: we may create any suffix we desire, ie testnet, near, etc.
    // but only those two (.testnet, .near) will be used in practice.
    let testnet = master_account.create_user("testnet".into(), ntoy(1_000_000_000));
    // We need an account to deploy the contracts from. We may create subaccounts of "testnet" as follows:
    let owner = testnet.create_user("contract-owner".into(), ntoy(1_000_000));
    let treasury = testnet.create_user("treasury".into(), ntoy(1_000_000));
    let operator = testnet.create_user("operator".into(), ntoy(1_000_000));

    //create acc, deploy & init the main contract
    let divpool = deploy!(
      contract: DiversifiedPoolContract,
      contract_id: "divpool",
      bytes: &WASM_BYTES_DIV_POOL,
      // User deploying the contract
      signer_account: &owner,
      // DiversifiedPool.new(
        //   owner_account_id: AccountId,
        //   treasury_account_id: AccountId,
        //   operator_account_id: AccountId,
      deposit:500*NEAR,
      gas:25*TGAS,
      init_method:new(owner.account_id(), treasury.account_id(), operator.account_id())
      );

    //deploy all the staking pools
    let mut sp = Vec::with_capacity(4);
    for n in 0..=3 {
      let sp_contract =deploy_simulated_staking_pool(&master_account, &format!("sp{}",n), &owner.account_id());
      call(&owner,&sp_contract,"pause_staking","{}",0,10*TGAS);
      sp.push( sp_contract );
    }

    return Self {

      master_account,

      testnet,
      owner,
      treasury,
      operator,

      divpool,

      sp,

    }

  }

  pub fn sp_balance(&self, n:usize) -> u128 { self.sp[n].amount()+self.sp[n].locked() }
  
  pub fn show_sp_balance(&self, n:usize) { 
      let total = self.sp_balance(n);
      let staked =  view_u128(&self.sp[n],"get_total_staked_balance","{}");
      println!("sp{} amount: {}, staked:{}+unstk:{}", n, total, staked, total - staked ); 
  }

  pub fn show_sps_balance(&self){
    println!("--SPs balance");
    for n in 0..=3 { self.show_sp_balance(n) }
    println!("--------------");
  }

  //----------------
  fn show_account_info(&self, acc:&str) -> Value {
    let divpool = &self.divpool;
    let result = view!(divpool.get_account_info(acc.into()));
    print_vecu8(acc,&result.unwrap());
    //println!("Result: {:#?}", result.unwrap_json_value());
    return serde_json::from_str(std::str::from_utf8(&result.unwrap()).unwrap()).unwrap();
  }

}

pub fn show_balance(ua:&UserAccount) { println!("@{} balance: staked:{} unstk:{}", ua.account_id(), ua.locked(),ua.amount() ); }

// #[test]
// fn sim_bug() {
//     let master_account = init_simulator(None);
//     let testnet = master_account.create_user("testnet".into(), ntoy(1_000_000_000));

//     let test_staker = testnet.create_user("staker".to_string(), ntoy(500_000));
//     show_balance(&test_staker);
    
//     let get_epoch_acc = master_account.deploy(&WASM_BYTES_GET_EPOCH, String::from("get_epoch_acc"), SP_INITIAL_BALANCE);
//     let user_txn = master_account
//       .create_transaction(get_epoch_acc.account_id())
//         .function_call(
//           "new".into(), 
//           "{}".into(),
//           50*TGAS, 0)
//         .submit();

//     println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));

//     let sk = SecretKey::from_seed(KeyType::ED25519, "test");
  
//     //stake => 10K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(10_000),  sk.public_key())
//       .submit();
  
//     show_balance(&test_staker);
//     assert!(test_staker.locked() == ntoy(10_000));
  
//     //stake => 15K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(15_000),  sk.public_key())
//       .submit();
  
//       show_balance(&test_staker);
//       assert!(test_staker.locked() == ntoy(15_000));
    
//     println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));
      
//     //stake => down to 7K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(7_000),  sk.public_key())
//       .submit();
  
//     show_balance(&test_staker);

//     println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));

//     //do it 15 times
//     //in the sim => 3 blocks make and epoch
//     for n in 0..5 {
//       call(&test_staker,&get_epoch_acc,"set_i32", &format!(r#"{{"num":{}}}"#,n),0,10*TGAS);
//       println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));
//     }

//     show_balance(&test_staker);

//     //stake => down to 7K
//     test_staker
//       .create_transaction(test_staker.account_id())
//       .stake(ntoy(7_000),  sk.public_key())
//       .submit();

//     assert!(test_staker.locked() == ntoy(7_000));
// }


#[test]
fn simtest() {
  
  let sim = Simulation::new();

  let divpool = &sim.divpool;

  let view_results = view!(divpool.get_contract_info());
  print_vecu8("contract_info",&view_results.unwrap());


  //Example transfer to account
  // let transaction = master_account
  //   .create_transaction("sp1".to_string());  
    //["sp1",".", &divpool_contract.user_account.account_id()].concat());
  //let res = transaction.transfer(ntoy(1)).submit();
  //print_helper(res);

  //test sp1 exists
  //println!("sp0 owner {}",view_call(&sim.sp[0], "get_owner_id", "{}"));

  // test yton & ntoy
  // println!("test: {}", yton(1*NEAR));
  // println!("test: {}", yton(10*NEAR));
  // println!("test: {}", yton(123*NEAR));
  // println!("test: {}", yton(ntoy(1)));
  // println!("test: {}", yton(ntoy(10)));
  // println!("test: {}", yton(ntoy(123)));

  //println!("treasury amount: {}", sim.treasury.amount());

  //---- register staking pools in the divpool contract
  let weight_basis_points_vec = vec!(15,40,25,20);
  for n in 0..sim.sp.len() {
    call!(sim.owner, divpool.set_staking_pool(sim.sp[n].account_id(),weight_basis_points_vec[n]*100), gas=25*TGAS);
  }
  let total_w_bp = view!(divpool.sum_staking_pool_list_weight_basis_points());
  assert!(total_w_bp.unwrap_json_value() == 10000);

  //---- alice
  //---- deposit & buy skash
  let alice = sim.testnet.create_user("alice".to_string(), ntoy(500_000));
  let alice_dep_and_stake = ntoy(100_000);
  let ads_res = call!(alice,divpool.deposit_and_stake(), alice_dep_and_stake, 50*TGAS);
  //print_helper(&ads_res);
  assert!(divpool.user_account.amount()>=alice_dep_and_stake);

  //---- bob
  let bob = sim.testnet.create_user("bob".to_string(), ntoy(500_000));
  let bob_dep_and_stake = ntoy(200_000);
  let bds_res = call!(bob,divpool.deposit_and_stake(), bob_dep_and_stake, 50*TGAS);

  //---- carol
  let carol = sim.testnet.create_user("carol".to_string(), ntoy(500_000));
  let carol_deposit = ntoy(250_000);
  let cd_res = call!(carol,divpool.deposit(), carol_deposit, 50*TGAS);
  println!("----------------------------------");
  println!("------- carol adds liquidiy --");
  let cal_res = call!(carol,divpool.nslp_add_liquidity(U128::from(carol_deposit)), gas=50*TGAS);

  //contract state
  let view_results = view!(divpool.get_contract_state());
  print_vecu8("contract_state",&view_results.unwrap());

  //---- test distribute_staking
  sim.show_sps_balance();
  println!("----------------------------------");
  println!("------- test distribute_staking --");
  for n in 0..4 {
    println!("------- call #{} to distribute_staking",n);
    let dres = call!(sim.operator, divpool.distribute_staking(), gas=125*TGAS );
    //print_helper_profile(&dres);
    sim.show_sps_balance();
  }
  
  //check the staking was distributed according to weight
  let total_staked = alice_dep_and_stake + bob_dep_and_stake;
  for n in 0..sim.sp.len() {
    let expected:u128 = SP_INITIAL_BALANCE + total_staked * weight_basis_points_vec[n] as u128 / 100;
    assert!( &sim.sp_balance(n) == &expected,
      "total_for_staking:{}, sp{} balance = {}, wbp:{}, !== expected:{}", alice_dep_and_stake, n, &sim.sp_balance(n), weight_basis_points_vec[n], expected);
  }

  //test unstake
  // let unstkres = view(&sim.sp[0],"unstake_all","{}",0,50*TGAS);
  // print_helper_promise(&unstkres);
  // sim.show_sps_balance();

  //----------------------------------------------------------
  sim.show_account_info(&alice.account_id());

  //----------------------------------------------------------
  println!("----------------------------------");
  println!("------- alice unstakes --");
  let alice_unstaking = ntoy(6_000);
  let ads_res = call!(alice,divpool.unstake(U128::from(alice_unstaking)), gas=50*TGAS);
  print_helper(&ads_res);

  //----------------------------------------------------------
  sim.show_account_info(&alice.account_id());

  //----------------------------------------------------------
  //---- test distribute_unstaking
  println!("----------------------------------");
  println!("------- test distribute_unstaking --");
  for n in 0..20 {
    println!("------- call #{} to distribute_unstaking",n);
    let dres = call!(sim.operator, divpool.distribute_unstaking(), gas=125*TGAS );
    print_helper_profile(&dres);
    sim.show_sps_balance();
    if &dres.unwrap_json_value()==false { break };
  }

  //deploy a contract to get the current epoch
  let get_epoch_acc = sim.master_account.deploy(&WASM_BYTES_GET_EPOCH, String::from("get_epoch_acc"), SP_INITIAL_BALANCE);
  let user_txn = sim.master_account
    .create_transaction(get_epoch_acc.account_id())
      .function_call(
        "new".into(), 
        "{}".into(),
        50*TGAS, 0)
      .submit();

  //----------------------------------------------------------
  //---- test retrieve unstaked funds
  println!("----------------------------------");
  println!("------- test retrieve funds from the pools --");
  for n in 0..30 {
    
    println!("epoch {}",view(&get_epoch_acc,"get_epoch_height","{}"));

    println!("------- call #{} to get_staking_pool_requiring_retrieve()",n);
    let dres = view!(divpool.get_staking_pool_requiring_retrieve());
    let inx = dres.unwrap_json_value().as_i64().unwrap();
    println!("------- result {}",inx);

    if inx>=0 {
      println!("------- pool #{} requires retrieve",inx);
      sim.show_sps_balance();
      let dres2 = call!(sim.operator, divpool.retrieve_funds_from_a_pool(inx as u16), gas=125*TGAS );
      print_helper_promise(&dres2);
    }
    else if inx==-3 { //no more funds unstaked
      break;
    }

    //make a dummy txn to advance the epoch
    call(&sim.owner, &get_epoch_acc,"set_i32",&format!(r#"{{"num":{}}}"#,inx).to_string(),0,10*TGAS);
  }

  //----------------------------------------------------------
  {
    println!("----------------------------------");
    println!("------- alice completes unstaking: withdraws --");
    let previous = alice.amount();
    let ads_res = call!(alice,divpool.withdraw(U128::from(alice_unstaking)), gas=50*TGAS);
    print_helper(&ads_res);
    assert!(alice.amount()==previous+alice_unstaking,"withdraw failed {} {} {}",alice.amount(),previous,alice_unstaking);
  }


  //----------------------------------------------------------
  {
    println!("----------------------------------");
    println!("------- bob sells skash (immediate unstake)");

    sim.show_account_info(&bob.account_id());
    sim.show_account_info(&carol.account_id());
    sim.show_account_info(NSLP_INTERNAL_ACCOUNT);
    let vr1 = view!(divpool.get_contract_state());
    print_vecu8("contract_state",&vr1.unwrap());
    let vr2 = view!(divpool.get_contract_params());
    print_vecu8("contract_params",&vr2.unwrap());
    

    let previous = bob.amount();
    const TO_SELL:u128 = 20_000*NEAR;
    const MIN_REQUESTED:u128 = 19_300*NEAR; //7% discount
    
    let dbp = view!(divpool.nslp_get_discount_basis_points(TO_SELL.into()));
    print_vecu8("divpool.nslp_get_discount_basis_points",&dbp.unwrap());

    let bss_res = call!(bob,divpool.sell_skash(U128::from(ntoy(20_000)),U128::from(MIN_REQUESTED)), gas=100*TGAS);
    print_helper(&bss_res);
    let received = as_u128(&bss_res.unwrap_json_value());
    assert!(received >= MIN_REQUESTED,"sell skash failed {} {}",MIN_REQUESTED,received);

    let bob_info = sim.show_account_info(&bob.account_id());
    let carol_info =sim.show_account_info(&carol.account_id());
    let nslp_info = sim.show_account_info(NSLP_INTERNAL_ACCOUNT);

    assert!(as_u128(&bob_info["g_skash"]) == 668*E24);
    assert!(as_u128(&carol_info["g_skash"]) == 9_352*E24);
    
  }

  //----------------------------------------------------------
  {
    println!("----------------------------------");
    const AMOUNT:u128 = 100_000*NEAR;
    println!("------- carol removes liquidity");
    let res = call!(carol,divpool.nslp_remove_liquidity(U128::from(AMOUNT)), gas=100*TGAS);
    print_helper(&res);
    let carol_info =sim.show_account_info(&carol.account_id());
    assert!(as_u128(&carol_info["total"]) == AMOUNT);
  }


}
