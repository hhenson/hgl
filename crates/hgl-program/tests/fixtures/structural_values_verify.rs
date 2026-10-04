assert_eq!(fixed.iter().map(|v|v.0.micros()).collect::<Vec<_>>(),vec![1,2,3]);
assert_eq!(fixed[0].1,(vec![0,2],vec![10,30]));
assert_eq!(fixed[1].1,(vec![0],vec![10]));
assert_eq!(fixed[2].1,(vec![1],vec![20]));
assert_eq!(tupled[0].1,(vec![1],vec!["first".to_owned()]));
assert_eq!(tupled[1].1,(vec![1],Vec::<String>::new()));
assert_eq!(quoted[0].1,(vec![4],vec!["original".to_owned()]));
assert_eq!(quoted[1].1,(vec![5],Vec::<String>::new()));
assert_eq!(quoted[2].1,(Vec::<i64>::new(),vec![String::new()]));
assert_eq!(membership[0].1,(vec![false,true],Vec::<bool>::new()));
assert_eq!(membership[1].1,(Vec::<bool>::new(),vec![false]));
assert_eq!(membership[2].1,(vec![false],vec![true]));
assert_eq!(nested.len(),3);
assert_eq!(nested[0].1.0,vec![7]);
assert_eq!(nested[0].1.1[0].0[0].0,vec![1]);
assert_eq!(nested[0].1.1[0].0[0].1[0],(vec![1],Vec::<String>::new()));
assert_eq!(nested[0].1.1[0].1[0],(vec![false],Vec::<bool>::new()));
assert_eq!(nested[1].1.1[0].0[0].1[0],(Vec::<i64>::new(),vec!["later".to_owned()]));
assert!(nested[1].1.1[0].1.is_empty());
assert!(nested[2].1.0.is_empty());
assert!(nested[2].1.1.is_empty());
assert_eq!(nested[2].1.2,vec![7]);
assert_eq!(scalar8.len(),2);
assert_eq!(scalar8[0].1,scalar8[1].1);
assert_eq!(scalar8[0].1.0,vec![false]);
assert_eq!(scalar8[0].1.3,vec![String::new()]);
// Changing every source and replacing the global entry cannot mutate this retained owner.
store.global_state().set(fixed_handle,&Vec::new()).unwrap();
assert_eq!(fixed[0].1.1,vec![10,30]);

assert!(empty.0.is_empty() && empty.1.is_empty());

assert_eq!(repeated.len(),1);
assert_eq!(repeated[0].1,(vec![2],vec!["paired".to_owned()]));
