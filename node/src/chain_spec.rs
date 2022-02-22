use node_template_runtime::{
	opaque::SessionKeys, AccountId, AuraConfig, Balance, BalancesConfig, GenesisConfig,
	GrandpaConfig, MaxNominations, SessionConfig, Signature, StakingConfig, SudoConfig,
	SystemConfig, DOLLARS, WASM_BINARY,
};
use pallet_staking::StakerStatus;
use rand::{distributions::Alphanumeric, rngs::OsRng, seq::SliceRandom, Rng};
use sc_service::ChainType;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::traits::{IdentifyAccount, Verify};

// The URL for the telemetry server.
// const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";

/// Specialized `ChainSpec`. This is a specialization of the general Substrate ChainSpec type.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

/// Generate a crypto pair from seed.
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate an Aura authority key.
pub fn authority_keys_from_seed(s: &str) -> (AccountId, AuraId, GrandpaId) {
	(
		// used as both stash and controller.
		get_account_id_from_seed::<sr25519::Public>(s),
		get_from_seed::<AuraId>(s),
		get_from_seed::<GrandpaId>(s),
	)
}

pub fn development_config() -> Result<ChainSpec, String> {
	let wasm_binary = WASM_BINARY.ok_or_else(|| "Development wasm not available".to_string())?;
	let rand_str =
		|| -> String { OsRng.sample_iter(&Alphanumeric).take(32).map(char::from).collect() };

	const N: usize = 1000;
	const A: usize = 100;

	println!("generating {} nominators and {} authorities.", N, A);

	let nominators = (0..N)
		.map(|_| rand_str())
		.map(|seed| get_account_id_from_seed::<sr25519::Public>(seed.as_str()))
		.collect::<Vec<_>>();

	let authorities = vec![authority_keys_from_seed("Alice")]
		.into_iter()
		.chain(
			(N..N + A)
				.map(|_| rand_str())
				.map(|seed| authority_keys_from_seed(seed.as_str())),
		)
		.collect::<Vec<_>>();

	let sudo = authorities.first().map(|x| x.0.clone()).unwrap();

	Ok(ChainSpec::from_genesis(
		// Name
		"Development",
		// ID
		"dev",
		ChainType::Development,
		move || {
			testnet_genesis(
				wasm_binary,
				authorities.clone(),
				nominators.clone(),
				sudo.clone(),
				true,
			)
		},
		vec![],
		None,
		None,
		None,
		None,
		None,
	))
}

fn session_keys(aura: AuraId, grandpa: GrandpaId) -> SessionKeys {
	SessionKeys { grandpa, aura }
}

enum NominationDegree {
	Partial,
	Full,
}

/// Configure initial storage state for FRAME modules.
fn testnet_genesis(
	wasm_binary: &[u8],
	initial_authorities: Vec<(AccountId, AuraId, GrandpaId)>,
	initial_nominators: Vec<AccountId>,
	root_key: AccountId,
	_enable_println: bool,
) -> GenesisConfig {
	const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
	const STASH_MIN: Balance = ENDOWMENT / 1000;
	const STASH_MAX: Balance = ENDOWMENT / 10;
	const ND: NominationDegree = NominationDegree::Full;

	let endowed_accounts = initial_authorities
		.iter()
		.map(|x| x.0.clone())
		.chain(initial_nominators.iter().cloned())
		.collect::<Vec<_>>();

	let mut rng = rand::thread_rng();
	let stakers = initial_authorities
		.iter()
		.map(|x| (x.0.clone(), x.0.clone(), STASH_MAX, StakerStatus::Validator))
		.chain(initial_nominators.iter().map(|x| {
			let limit = (MaxNominations::get() as usize).min(initial_authorities.len());
			let count = match ND {
				NominationDegree::Full => (rng.gen::<usize>() % limit).max(1),
				NominationDegree::Partial => limit,
			};

			let nominations = initial_authorities
				.as_slice()
				.choose_multiple(&mut rng, count)
				.into_iter()
				.map(|choice| choice.0.clone())
				.collect::<Vec<_>>();
			(
				x.clone(),
				x.clone(),
				rng.gen_range(STASH_MIN..=STASH_MAX),
				StakerStatus::Nominator(nominations),
			)
		}))
		.collect::<Vec<_>>();

	GenesisConfig {
		system: SystemConfig { code: wasm_binary.to_vec() },
		balances: BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, ENDOWMENT)).collect(),
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), session_keys(x.1.clone(), x.2.clone())))
				.collect::<Vec<_>>(),
		},
		staking: StakingConfig {
			stakers,
			validator_count: initial_authorities.len() as u32,
			minimum_validator_count: (initial_authorities.len() / 2) as u32,
			..Default::default()
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		sudo: SudoConfig { key: Some(root_key) },
		transaction_payment: Default::default(),
	}
}
