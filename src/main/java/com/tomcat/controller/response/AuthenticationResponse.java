package com.tomcat.controller.response;

public class AuthenticationResponse {

  private String accessToken;
  private String tokenType = "Bearer";

  public AuthenticationResponse(String accessToken) {
    this.accessToken = accessToken;
  }

  // getter和setter

  public String getAccessToken() {
    return accessToken;
  }

  public void setAccessToken(String accessToken) {
    this.accessToken = accessToken;
  }

  public String getTokenType() {
    return tokenType;
  }

  public void setTokenType(String tokenType) {
    this.tokenType = tokenType;
  }
}