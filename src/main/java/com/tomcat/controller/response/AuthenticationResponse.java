package com.tomcat.controller.response;

import com.tomcat.domain.User;

public class AuthenticationResponse {

  private String accessToken;
  private String tokenType = "Bearer";

  private User user;

  public AuthenticationResponse(String accessToken, User user) {
    this.accessToken = accessToken;
    this.user = user;
  }

  public AuthenticationResponse(String jwtToken) {
    this.accessToken = jwtToken;
  }

  // getter和setter


  public User getUser() {
    return user;
  }

  public void setUser(User user) {
    this.user = user;
  }

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