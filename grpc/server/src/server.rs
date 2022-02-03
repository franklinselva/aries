use anyhow::{Context, Result};
use async_trait::async_trait;
use tonic::{transport::Server, Request, Response, Status};

mod solver;
use solver::solve;
// use crate::solver::*;

use aries_grpc_api::upf_server::{Upf, UpfServer};
use aries_grpc_api::{Answer, Problem};

#[derive(Default)]
pub struct UpfService {}

#[async_trait]
impl Upf for UpfService {
    async fn plan(&self, request: Request<Problem>) -> Result<Response<Answer>, Status> {
        let problem = request.into_inner();

        // let problem = Problem_::deserialize(problem);
        println!("{:?}", problem);
        let _answer = solve(problem).with_context(|| format!("Unable to solve the problem"));
        let answer = Answer::default();

        Ok(Response::new(answer))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set address to localhost
    let addr = "127.0.0.1:2222".parse()?;
    let upf_service = UpfService::default();

    Server::builder()
        .add_service(UpfServer::new(upf_service))
        .serve(addr)
        .await?;

    Ok(())
}
