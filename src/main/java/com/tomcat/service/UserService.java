package com.tomcat.service;

import com.tomcat.controller.request.AuthenticationRequest;
import com.tomcat.controller.response.AuthenticationResponse;

public interface UserService {
  AuthenticationResponse register(AuthenticationRequest request);
  
  AuthenticationResponse login(AuthenticationRequest request);
}